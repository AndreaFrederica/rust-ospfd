use ospf_packet::{
    lsa::{
        link_types::{STUB_LINK, TRANSIT_LINK},
        types::ROUTER_LSA,
        Lsa, LsaHeader, RouterLSA, RouterLSALink,
    },
    packet::options,
};

use crate::{
    constant::{InitialSequenceNumber, MaxSequenceNumber},
    database::{InterfacesGuard, ProtocolDB},
    flooding::flooding,
    interface::{InterfaceState, NetType},
    log_warning, must,
    neighbor::NeighborState,
};

pub async fn gen_router_lsa(interfaces: &mut InterfacesGuard) {
    let mut header = LsaHeader {
        ls_age: 0,
        options: 0,
        ls_type: ROUTER_LSA,
        link_state_id: ProtocolDB::get_router_id(),
        advertising_router: ProtocolDB::get_router_id(),
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
            // 如果接口状态为 Waiting，加入类型 3 连接（存根网络）。
            if iface.state == InterfaceState::Waiting {
                lsa.links.push(RouterLSALink {
                    link_id: iface.ip_addr,
                    link_data: iface.ip_mask,
                    link_type: STUB_LINK,
                    tos: 0,
                    metric: 0,
                });
            } else if iface.is_dr() && !iface.neighbors.is_empty()
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
                    metric: 1,
                });
            }
        }
    }
    lsa.num_links = lsa.links.len() as u16;
    let mut lsa: Lsa = (header, lsa).try_into().unwrap();
    lsa.update_length();
    lsa.update_checksum();
    // todo! temporary ignore identical lsa
    if let Some((old, ..)) = old {
        if old.data == lsa.data {
            // identical
            return;
        }
    }
    ProtocolDB::get()
        .await
        .insert_lsa(interfaces.me.area_id, lsa.clone())
        .await;
    flooding(interfaces, interfaces.me.ip_addr, &lsa).await;
}
