use ospf_packet::packet::DBDescription;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: DBDescription) {
    todo!("iface: {:?}, src: {:?}, packet: {:?}", iface, src, packet);
}
