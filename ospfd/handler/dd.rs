use ospf_packet::packet::DBDescription;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: DBDescription) {
    let name = iface.read().await.interface_name.to_string();
    todo!("iface: {:?}, src: {:?}, packet: {:?}", name, src, packet);
}
