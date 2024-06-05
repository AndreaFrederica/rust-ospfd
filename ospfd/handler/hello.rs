use ospf_packet::packet::HelloPacket;

use crate::{interface::AInterface, neighbor::ANeighbor};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: HelloPacket) {
    //todo 配置验证
    let neighbor = src.write().await;
    let mut iface = iface.write().await;
    iface.neighbors.push(neighbor.router_id);
    iface.neighbors.sort();
    iface.neighbors.dedup();
    //todo state change
    todo!("packet: {:?}", packet)
}
