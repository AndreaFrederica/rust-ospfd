use ospf_packet::packet::LSRequest;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: LSRequest) {
    let name = iface.read().await.interface_name.to_string();
    todo!("iface: {:?}, src: {:?}, packet: {:?}", name, src, packet);
}
