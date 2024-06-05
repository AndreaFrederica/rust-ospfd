use std::time::Duration;

use ospf_packet::packet::HelloPacket;
use tokio::time::sleep;

use crate::{
    interface::{AInterface, InterfaceEvent},
    neighbor::{ANeighbor, NeighborEvent},
};
use futures::FutureExt;

pub async fn handle(iface: AInterface, src: ANeighbor, packet: HelloPacket) {
    //todo 配置验证
    iface
        .write()
        .await
        .neighbors
        .insert(src.read().await.router_id, src.clone());
    let iface_cloned = iface.clone();
    iface
        .write()
        .await
        .wait_timer
        .replace(tokio::spawn(
            sleep(Duration::from_secs(1)).then(|_| iface_cloned.wait_timer()),
        ))
        .map(|f| f.abort());
    //todo state change
    src.write().await.dead_interval = packet.router_dead_interval;
    src.clone().hello_receive().await;
    todo!("hello packet")
}
