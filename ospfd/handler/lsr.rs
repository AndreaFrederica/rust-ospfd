use ospf_macros::define;
use ospf_packet::packet::{LSRequest, LSUpdate};

use crate::{
    database::{LsaIndex, ProtocolDB},
    guard, must,
    neighbor::{NeighborEvent, NeighborState, RefNeighbor},
    sender::send_packet,
};

#[define(iface => src.get_interface(); neighbor => src.get_neighbor())]
pub async fn handle(mut src: RefNeighbor<'_>, packet: LSRequest) {
    must!(neighbor.state >= NeighborState::Exchange);
    guard! {
        Some(lsa) = ProtocolDB::get().await.get_lsa(
            iface.area_id,
            LsaIndex::new(
                packet.ls_type as u8,
                packet.ls_id,
                packet.advertising_router
            )
        ).await;
        else: src.bad_ls_req().await;
    };
    let packet = LSUpdate {
        num_lsa: 1,
        lsa: vec![lsa],
    };
    let ip = neighbor.ip_addr;
    send_packet(iface, &packet, ip).await
}
