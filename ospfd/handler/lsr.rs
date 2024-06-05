use ospf_packet::packet::LSRequest;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: LSRequest) {
    todo!("iface: {:?}, src: {:?}, packet: {:?}", iface, src, packet);
}
