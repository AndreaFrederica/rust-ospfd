use std::ops::Deref;

use ospf_packet::packet::{self, options::OptionExt, HelloPacket};

use crate::{
    interface::{AInterface, InterfaceEvent, InterfaceState, NetType},
    must,
    neighbor::{ANeighbor, NeighborEvent, NeighborSubStruct},
    util::hex2ip,
};

pub async fn handle(iface: AInterface, src: ANeighbor, packet: HelloPacket) {
    {
        // must
        let iface = iface.read().await;
        must!(iface.hello_interval == packet.hello_interval);
        must!(iface.dead_interval == packet.router_dead_interval);
        must!(
            matches!(iface.net_type, NetType::P2P | NetType::Virtual)
                || iface.ip_mask == packet.network_mask
        );
        must!(iface.external_routing == packet.is_set(packet::options::E));
    }
    let prev_state: NeighborSubStruct = src.read().await.deref().into();
    {
        // neighbor structure
        let mut neighbor = src.write().await;
        neighbor.option = packet.options;
        neighbor.dr = packet.designated_router;
        neighbor.bdr = packet.backup_designated_router;
        neighbor.priority = packet.router_priority;
    }
    // insert neighbor
    iface
        .write()
        .await
        .neighbors
        .insert(prev_state.ip_addr, src.clone());
    // 每个 Hello 包引起邻居状态机执行事件 HelloReceived
    src.clone().hello_receive().await;
    // 如果路由器自身出现在列表中，邻居状态机执行事件 2-WayReceived
    // 否则，邻居状态机执行事件 1-WayReceived，并终止包处理过程
    if packet.neighbors.contains(&iface.read().await.router_id) {
        src.clone().two_way_received().await;
    } else {
        src.clone().one_way_received().await;
        return;
    }
    // 如果发现邻居优先级有改变，接收接口状态机调度执行事件 NeighborChange
    if packet.router_priority != prev_state.priority {
        iface.clone().neighbor_change().await;
    }
    if packet.designated_router == prev_state.ip_addr
        && packet.backup_designated_router == hex2ip(0)
        && iface.read().await.state == InterfaceState::Waiting
    {
        // 如果邻居宣告自己为 DR && 而且接收接口状态机的状态为 Waiting
        // 接收接口状态机调度执行事件 BackupSeen
        iface.clone().backup_seen().await;
    } else if packet.designated_router == prev_state.ip_addr && !prev_state.is_dr()
        || prev_state.is_dr() && packet.designated_router != prev_state.ip_addr
    {
        // 如果以前不宣告的邻居宣告自己为 DR，或以前宣告的邻居现在不宣告自己为 DR
        // 接收接口状态机调度执行事件 NeighborChange
        iface.clone().neighbor_change().await;
    }
    if packet.backup_designated_router == prev_state.ip_addr
        && iface.read().await.state == InterfaceState::Waiting
    {
        // 如果邻居宣告自己为 BDR && 而且接收接口状态机的状态为 Waiting
        // 接收接口状态机调度执行事件 BackupSeen
        iface.clone().backup_seen().await;
    } else if packet.backup_designated_router == prev_state.ip_addr && !prev_state.is_bdr()
        || prev_state.is_bdr() && packet.backup_designated_router != prev_state.ip_addr
    {
        // 如果以前不宣告的邻居宣告自己为 DR，或以前宣告的邻居现在不宣告自己为 DR
        // 接收接口状态机调度执行事件 NeighborChange
        iface.clone().neighbor_change().await;
    }
}
