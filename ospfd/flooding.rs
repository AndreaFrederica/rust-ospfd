use std::{cmp::Ordering, net::Ipv4Addr};

use ospf_packet::{
    lsa::{types::AS_EXTERNAL_LSA, Lsa, LsaIndex},
    packet::LSUpdate,
};

use crate::{
    constant::{AllDRouters, AllSPFRouters, LsaMaxAge},
    database::InterfacesGuard,
    interface::Interface,
    must,
    sender::send_packet,
};

pub async fn flooding(interfaces: &mut InterfacesGuard, src_ip: Ipv4Addr, lsa: &Lsa) -> bool {
    let lsa_area = interfaces.me.area_id;
    let me = interfaces.me.ip_addr;
    // 合格接口
    let ac_iface = interfaces.iter_mut().filter(|iface| {
        if lsa.header.ls_type == AS_EXTERNAL_LSA {
            iface.external_routing
        } else {
            iface.area_id == lsa_area
        }
    });
    // 逐个接口处理
    let rt = tokio::runtime::Handle::current();
    let result: Vec<_> = tokio::task::block_in_place(|| {
        ac_iface
            .map(|mut i| rt.block_on(flooding_on(&mut i, me, src_ip, lsa)))
            .collect()
    });
    result.into_iter().any(|b| b)
}

async fn flooding_on(iface: &mut Interface, me: Ipv4Addr, src: Ipv4Addr, lsa: &Lsa) -> bool {
    let mut success = false;
    // （1）检查接口上的各个邻居，判断是否必须接收新的 LSA，对每个邻居执行下面的步骤：
    for neighbor in iface.neighbors.values_mut() {
        // （a）如果邻居状态小于 Exchange，它不参与洪泛，检查下一个邻居。
        must!(neighbor.state >= crate::neighbor::NeighborState::Exchange; continue);
        // （b）如果邻接还没有完全，检查邻接所关联的连接状态请求列表。如果存在有该 LSA 的实例，表示邻居已经有了该 LSA。
        if let Some(index) = neighbor
            .ls_request_list
            .iter()
            .position(|&h| LsaIndex::from(h) == lsa.header.into())
        {
            match lsa.header.cmp(&neighbor.ls_request_list[index]) {
                // 如果新的 LSA 较老，检查下一个邻居
                Ordering::Less => continue,
                // 如果两个副本为相同实例，删除连接状态请求列表中的 LSA，检查下一个邻居。
                Ordering::Equal => {
                    neighbor.ls_request_list.swap_remove_back(index);
                    neighbor.lsr_handle.child_abort();
                    continue;
                }
                // 否则，如果新的 LSA 较新，删除连接状态请求列表中的 LSA。
                Ordering::Greater => {
                    neighbor.ls_request_list.swap_remove_back(index);
                    neighbor.lsr_handle.child_abort();
                }
            }
        }
        // （c）如果新的 LSA 是从该邻居所接收，检查下一个邻居。
        must!(src != neighbor.ip_addr; continue);
        // （d）这时，如果不能肯定邻居有 LSA 的最新实例，将新的 LSA 加到邻接的连接状态重传列表中。
        neighbor.ls_retransmission_list.insert(lsa.header.into());
        success = true;
    }
    // （2）如果在上一步中，”没有”向连接状态重传列表加入任何 LSA，就不需要将 LSA 洪泛出接口。检查下一个接口。
    must!(success; ret: false);
    // （3/4）如果 LSA 是由该接口接收。
    if iface.ip_addr == me && iface.ip_addr != src {
        // （3）且是从 DR 或 BDR 接收到的，说明其他邻居都已经接收到该 LSA。检查下一个接口
        let neighbor = iface.neighbors.get(&src).unwrap();
        if neighbor.is_dr() || neighbor.is_bdr() {
            return false;
        }
        // （4）且接口状态为 Backup（路由器是 BDR），检查下一个接口。
        if iface.is_bdr() {
            return false;
        }
    }
    // （5）如果到达这步，接口必须洪泛该 LSA。发送一个 LSU 包（包含新 LSA）出接口。
    //     当复制该 LSA 时，其 LS 时限必须增加 InfTransDelay（直到 LS 时限域达到 MaxAge）
    let mut lsa = lsa.clone();
    lsa.header.ls_age += iface.inf_trans_delay;
    lsa.header.ls_age = lsa.header.ls_age.min(LsaMaxAge);
    let packet = LSUpdate {
        num_lsa: 1,
        lsa: vec![lsa],
    };
    let dest = if iface.is_drother() {
        AllDRouters
    } else {
        AllSPFRouters
    };
    send_packet(iface, &packet, dest).await;
    true
}
