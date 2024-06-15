use std::{net::Ipv4Addr, ops::ControlFlow};

use ospf_packet::{
    lsa::{
        types::{AS_EXTERNAL_LSA, NETWORK_LSA},
        Lsa, LsaHeader, LsaIndex,
    },
    packet::{LSAcknowledge, LSUpdate},
};

use crate::{
    constant::{AllDRouters, AllSPFRouters, LsaMaxAge, MaxSequenceNumber, MinLSArrival},
    database::{InterfacesGuard, ProtocolDB},
    handler::flooding::flooding,
    interface::InterfaceState,
    log_error, log_warning, must,
    neighbor::{NeighborEvent, NeighborState, RefNeighbor},
    sender::send_packet,
};

macro_rules! ret {
    (continue) => {
        ControlFlow::Continue(())
    };
    (break) => {
        ControlFlow::Break(())
    };
}

pub async fn handle(interfaces: InterfacesGuard, src_ip: Ipv4Addr, packet: LSUpdate) {
    let mut delay = vec![];
    let mut meta = Metadata(interfaces, src_ip);
    for lsa in packet.lsa {
        match handle_one(&mut meta, lsa, &mut delay).await {
            ret!(continue) => continue,
            ret!(break) => break,
        }
    }
    if !delay.is_empty() {
        let dest = if meta.0.me.is_drother() {
            AllDRouters
        } else {
            AllSPFRouters
        };
        let packet = LSAcknowledge { lsa_header: delay };
        send_packet(&mut meta.0.me, &packet, dest).await;
    }
}

struct Metadata(InterfacesGuard, Ipv4Addr);

impl Metadata {
    fn get_neighbor(&mut self) -> RefNeighbor<'_> {
        RefNeighbor::from(&mut self.0.me, self.1).unwrap()
    }
}

macro_rules! invoke {
    ($meta:ident.$func:ident, $param:expr) => {
        ProtocolDB::get()
            .await
            .$func($meta.0.me.area_id, $param)
            .await
    };
}

macro_rules! neighbor {
    ($meta:ident) => {
        $meta.get_neighbor().get_neighbor()
    };
}

async fn handle_one(
    meta: &mut Metadata,
    lsa: Lsa,
    vec: &mut Vec<LsaHeader>,
) -> ControlFlow<(), ()> {
    // 1. 确认 LSA 的 LS 校验和。
    must!(lsa.checksum_ok(); ret: ret!(continue));
    // 2. 检查 LSA 的 LS 类型。
    must!(matches!(lsa.header.ls_type, 1..=5); ret: ret!(continue));
    // 3. 如果是一个 AS-external-LSA
    must!(!matches!(lsa.header.ls_type, AS_EXTERNAL_LSA) || meta.0.me.external_routing; ret: ret!(continue));
    // special: 如果这是邻居对我的 lsr 的回应
    if let Some(header) = neighbor!(meta).ls_request_list.front() {
        if LsaIndex::from(lsa.header) == LsaIndex::from(*header) {
            meta.get_neighbor().lsr_recv_update();
            invoke!(meta.insert_lsa, lsa);
            return ret!(continue);
        }
    }
    // 4. 如果 LSA 的 LS 时限等于 MaxAge, 而且路由器的连接状态数据库中没有该
    //    LSA 的实例，而且路由器的邻居都不处于 Exchange 或 Loading 状态
    if lsa.header.ls_age == LsaMaxAge
        && !invoke!(meta.contains_lsa, lsa.header.into())
        && meta.0.iter().all(|i| {
            i.neighbors
                .values()
                .all(|n| !matches!(n.state, NeighborState::Exchange | NeighborState::Loading))
        })
    {
        // a）通过发送一个 LSAck 包到发送的邻居（见第 13.5 节）来确认收到该 LSA
        let packet = LSAcknowledge {
            lsa_header: vec![lsa.header],
        };
        send_packet(&mut meta.0.me, &packet, meta.1).await;
        // b）丢弃该 LSA
        return ret!(continue);
    }
    // 5.否则，在路由器当前的连接状态数据库中查找该 LSA 的实例。如果没有找到数据库中的副本，或所接收的 LSA 比数据库副本新
    if invoke!(meta.need_update, lsa.header) {
        let db_lsa = invoke!(meta.get_lsa, lsa.header.into());
        // a）如果已经有一个数据库副本，而且是在 MinLSArrival 秒内通过洪泛而加入数据库的
        if db_lsa
            .as_ref()
            .is_some_and(|(_, t, _)| t.elapsed().as_secs() < MinLSArrival.into())
        {
            return ret!(continue);
        }
        // b）否则，立即将新 LSA 洪泛出路由器的某些接口（见第 13.3 节）
        let flood = flooding(&mut meta.0, meta.1, &lsa).await;
        // c）将当前数据库中的副本，从所有的邻居连接状态重传列表中删除。
        if let Some((db_lsa, ..)) = db_lsa.as_ref() {
            for iface in meta.0.iter_mut() {
                for neighbor in iface.neighbors.values_mut() {
                    neighbor
                        .ls_retransmission_list
                        .remove(&db_lsa.header.into());
                }
            }
        }
        // d) 将新的 LSA 加入连接状态数据库（取代当前数据库的副本），这可能导致按调度计算路由表
        invoke!(meta.insert_lsa, lsa.clone());
        log_warning!("todo: recalculate routing table");
        // e）也许需要从接收接口发送 LSAck 包以确认所收到的 LSA。这在第 13.5 节说明。
        if !flood {
            if meta.0.me.state != InterfaceState::Backup || neighbor!(meta).is_dr() {
                vec.push(lsa.header);
            }
        }
        // f）如果这个新的 LSA 是由路由器自身所生成的（即被作为自生成 LSA），
        //    路由器执行特殊的操作，或许更新该 LSA，或将其从路由域中删除。
        //    自生成 LSA 的删除以及随后操作的描述，见第 13.4 节。
        if lsa.header.advertising_router == ProtocolDB::get_router_id()
            || lsa.header.ls_type == NETWORK_LSA
                && meta.0.iter().any(|i| i.ip_addr == lsa.header.link_state_id)
        {
            log_error!("todo: deal with self created lsa");
        }
        return ret!(continue);
    }
    // 6.否则，如果该 LSA 的实例正在邻居的连接状态请求列表上，产生数据库交换过程的错误。
    //   生成该邻居的 BadLSReq 事件，重新启动数据库交换过程，并停止处理 LSU 包。
    if neighbor!(meta).ls_request_list.contains(&lsa.header) {
        meta.get_neighbor().bad_ls_req().await;
        return ret!(break);
    }
    // 7.否则，如果接收的 LSA 与数据库副本为同一实例（没有哪个较新）
    let (db_lsa, _, updated) = invoke!(meta.get_lsa, lsa.header.into()).unwrap();
    if db_lsa.header == lsa.header {
        // a）如果 LSA 在所接收邻居的连接状态重传列表上，表示路由器自身正期待着这一 LSA 的确认。
        //   路由器可以将这一 LSA 作为确认，并将其从连接状态重传列表中去除。这被称为”隐含确认”，
        //   这需要在后面的确认过程中注意（见第 13.5 节）。
        if neighbor!(meta)
            .ls_retransmission_list
            .remove(&lsa.header.into())
        {
            if meta.0.me.state == InterfaceState::Backup && neighbor!(meta).is_dr() {
                // send delay ls ack
                vec.push(lsa.header);
            }
        } else {
            // b）也许需要从接收接口发送 LSAck 包以确认所收到的 LSA。这在第 13.5 节说明。
            let packet = LSAcknowledge {
                lsa_header: vec![lsa.header],
            };
            send_packet(&mut meta.0.me, &packet, meta.1).await;
        }
        return ret!(continue);
    }
    // 8.否则，数据库中的副本较近。
    if db_lsa.header.ls_age == LsaMaxAge && db_lsa.header.ls_sequence_number == MaxSequenceNumber {
        // a）如果数据库副本的 LS 时限等于 MaxAge，且 LS 序号等于 MaxSequenceNumber，则丢弃接收到的 LSA 而不作确认
        ret!(continue)
    } else {
        // b）否则，只要数据库中的副本没有在最近 MinLSArrival 秒中被 LSU 包发送，就用一个 LSU 包发回给邻居。
        if updated.elapsed().as_secs() > MinLSArrival.into() {
            let packet = LSUpdate {
                num_lsa: 1,
                lsa: vec![db_lsa],
            };
            send_packet(&mut meta.0.me, &packet, meta.1).await;
        }
        ret!(continue)
    }
}
