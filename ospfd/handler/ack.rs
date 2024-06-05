use ospf_packet::packet::LSAcknowledge;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: LSAcknowledge) {
    todo!("iface: {:?}, src: {:?}, packet: {:?}", iface, src, packet);
}
