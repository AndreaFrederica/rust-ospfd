use ospf_macros::define;
use ospf_packet::packet::LSAcknowledge;

use crate::{must, neighbor::{NeighborState, RefNeighbor}};

#[define(iface => src.get_interface(); neighbor => src.get_neighbor())]
pub async fn handle(mut src: RefNeighbor<'_>, packet: LSAcknowledge) {
    must!(neighbor.state >= NeighborState::Exchange);
    for ack in packet.lsa_header {
        neighbor.ls_retransmission_list.remove(&ack.into());
    }
}
