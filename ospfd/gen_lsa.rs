use std::{marker::PhantomData, net::Ipv4Addr};

use ospf_packet::{
    lsa::{link_types::*, types::*, *},
    packet::options,
};

use crate::{
    constant::{BackboneArea, InitialSequenceNumber, LSInfinity, LsRefreshTime, MaxSequenceNumber},
    database::{InterfacesGuard, ProtocolDB, RoutingTableItemType, RoutingTablePathType},
    flooding::flooding,
    interface::{InterfaceState, NetType},
    log_warning, must,
    neighbor::NeighborState,
};

pub async fn gen_router_lsa(interfaces: &mut InterfacesGuard) {
    let mut lsa = RouterLSA::default();
    if ProtocolDB::get().await.areas.len() > 1 {
        lsa.b = 1; // ABR
    }
    if !ProtocolDB::get().await.external_routes.is_empty() {
        lsa.e = 1; // ASBR
    }
    // todo? V bit?
    for iface in interfaces.iter() {
        must!(iface.area_id == interfaces.me.area_id; continue);
        must!(iface.state != InterfaceState::Down; continue);
        if iface.state == InterfaceState::Loopback {
            log_warning!("todo: loopback interface");
        } else {
            assert!(matches!(iface.net_type, NetType::Broadcast | NetType::NBMA));
            if iface.is_dr() && !iface.neighbors.is_empty()
            // 如果路由器与 DR 完全邻接
            // 或路由器自身为 DR 且与至少一台其他路由器邻接
            // 加入类型 2 连接（传输网络）
                || iface
                    .neighbors
                    .get(&iface.dr)
                    .is_some_and(|n| n.state == NeighborState::Full)
            {
                lsa.links.push(RouterLSALink {
                    link_id: iface.dr,
                    link_data: iface.ip_addr,
                    link_type: TRANSIT_LINK,
                    tos: 0,
                    metric: iface.cost,
                });
            } else {
                // 否则，加入类型 3 连接（存根网络）
                lsa.links.push(RouterLSALink {
                    link_id: iface.ip_addr,
                    link_data: iface.ip_mask,
                    link_type: STUB_LINK,
                    tos: 0,
                    metric: iface.cost,
                });
            }
        }
    }
    lsa.num_links = lsa.links.len() as u16;
    let router_id = ProtocolDB::get_router_id();
    gen_lsa_impl(interfaces, ROUTER_LSA, router_id, router_id, lsa).await;
}

pub async fn gen_network_lsa(interfaces: &mut InterfacesGuard) {
    must!(interfaces.me.is_dr());
    let lsa = NetworkLSA {
        network_mask: interfaces.me.ip_mask,
        attached_routers: interfaces
            .me
            .neighbors
            .values()
            .filter(|n| n.state == NeighborState::Full)
            .map(|n| n.router_id)
            .chain(std::iter::once(ProtocolDB::get_router_id()))
            .collect(),
    };
    must!(lsa.attached_routers.len() > 1);
    let router_id = ProtocolDB::get_router_id();
    let ls_id = interfaces.me.ip_addr;
    gen_lsa_impl(interfaces, NETWORK_LSA, ls_id, router_id, lsa).await;
}

pub async fn gen_summary_lsa(interfaces: &mut InterfacesGuard) {
    let i_areas = interfaces
        .iter()
        .map(|i| i.area_id)
        .collect::<std::collections::HashSet<_>>();
    ProtocolDB::get()
        .await
        .areas
        .retain(|area_id, _| i_areas.contains(area_id));
    let db = ProtocolDB::get().await;
    let router_id = ProtocolDB::get_router_id();
    // 至少有两个区域
    must!(db.areas.len() > 1);
    use RoutingTablePathType::*;
    let routings: Vec<_> = db
        .routing_table
        .get_routings()
        .into_iter()
        // 决不在 Summary-LSA 中宣告 AS 外部路径
        .filter(|item| matches!(item.path_type, AreaInternal | AreaExternal))
        // 不生成距离值等于或超过 LSInfinity
        .filter(|item| item.cost < LSInfinity)
        .collect();
    let mut packets = vec![];
    for item in &routings {
        must!(interfaces.me.area_id != item.area_id; continue);
        must!(interfaces.me.area_id != BackboneArea || item.path_type == AreaInternal; continue);
        let lsa = SummaryLSA {
            network_mask: item.addr_mask,
            _zeros: PhantomData,
            metric: item.cost,
        };
        packets.push(match item.dest_type {
            RoutingTableItemType::Router => {
                must!(interfaces.me.external_routing; continue);
                (SUMMARY_ASBR_LSA, item.dest_id, lsa)
            }
            RoutingTableItemType::Network => {
                (SUMMARY_IP_LSA, item.dest_id, lsa)
            }
        });
    }
    drop(db);
    for (ls_type, link_state_id, lsa) in packets {
        gen_lsa_impl(interfaces, ls_type, link_state_id, router_id, lsa).await;
    }
}

/// 这个函数是一个模板。提供了 LsaHeader 的生成，以及和数据库的比对，和洪泛。
async fn gen_lsa_impl<T>(
    interfaces: &mut InterfacesGuard,
    ls_type: u8,
    link_state_id: Ipv4Addr,
    advertising_router: Ipv4Addr,
    lsa: T,
) where
    (LsaHeader, T): TryInto<Lsa, Error = ConvertError>,
{
    let mut header = LsaHeader {
        ls_age: 0,
        options: 0,
        ls_type,
        link_state_id,
        advertising_router,
        ls_sequence_number: InitialSequenceNumber,
        ls_checksum: 0,
        length: 0,
    };
    if interfaces.me.external_routing {
        header.options |= options::E;
    }
    let old = ProtocolDB::get()
        .await
        .get_lsa(interfaces.me.area_id, header.into())
        .await;
    if let Some((old, ..)) = old.as_ref() {
        //todo! if ls_sequence_number == MaxSequenceNumber
        assert_ne!(old.header.ls_sequence_number, MaxSequenceNumber);
        header.ls_sequence_number = old.header.ls_sequence_number + 1;
    }
    let mut lsa: Lsa = (header, lsa).try_into().unwrap();
    lsa.update_length();
    lsa.update_checksum();
    // todo! temporary ignore identical lsa
    if let Some((old, ..)) = old {
        if old.header.ls_age < LsRefreshTime && old.data == lsa.data {
            // identical
            return;
        }
    }
    ProtocolDB::get()
        .await
        .insert_lsa(interfaces.me.area_id, lsa.clone())
        .await;
    flooding(interfaces, interfaces.me.ip_addr, &lsa).await;
    ProtocolDB::get().await.recalc_routing().await;
}
