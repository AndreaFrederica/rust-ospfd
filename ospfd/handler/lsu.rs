use ospf_macros::define;
use ospf_packet::packet::LSUpdate;

use crate::{database::LsaIndex, log_error, neighbor::RefNeighbor};

#[define(iface => src.get_interface(); neighbor => src.get_neighbor())]
pub async fn handle(mut src: RefNeighbor<'_>, packet: LSUpdate) {
    for ref lsa in packet.lsa {
        if let Some(header) = neighbor.ls_request_list.front() {
            if LsaIndex::from(&lsa.header) == LsaIndex::from(header) {
                src.lsr_recv_update();
                log_error!("update database!!");
                continue;
            }
        }
        log_error!("todo: handle other lsu...");
    }
}
