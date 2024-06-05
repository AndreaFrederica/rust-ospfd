use ospf_packet::packet::LSUpdate;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: LSUpdate) {
    todo!("iface: {:?}, src: {:?}, packet: {:?}", iface, src, packet);
}
