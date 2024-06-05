use ospf_packet::packet::HelloPacket;

use crate::{interface::AInterface, neighbor::{ANeighbor, NeighborEvent}};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: HelloPacket) {
    //todo 配置验证
    iface.write().await.neighbors.insert(src.read().await.router_id, src.clone());
    src.clone().hello_receive().await;
    //todo state change
    todo!("packet: {:?}", packet)
}
