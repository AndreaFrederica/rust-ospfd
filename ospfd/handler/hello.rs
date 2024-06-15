use std::ops::Deref;

use ospf_macros::define;
use ospf_packet::packet::{self, options::OptionExt, HelloPacket};

use crate::{
    database::ProtocolDB,
    interface::{InterfaceEvent, InterfaceState, NetType},
    must,
    neighbor::{NeighborEvent, NeighborSubStruct, RefNeighbor},
    util::hex2ip,
};

#[define(iface => src.get_interface(); neighbor => src.get_neighbor())]
pub async fn handle(mut src: RefNeighbor<'_>, packet: HelloPacket) {
    // must
    must!(iface.hello_interval == packet.hello_interval);
    must!(iface.dead_interval == packet.router_dead_interval);
    must!(
        matches!(iface.net_type, NetType::P2P | NetType::Virtual)
            || iface.ip_mask == packet.network_mask
    );
    must!(iface.external_routing == packet.is_set(packet::options::E));
    // neighbor structure
    let prev_state = NeighborSubStruct::from(neighbor.deref());
    neighbor.option = packet.options;
    neighbor.dr = packet.designated_router;
    neighbor.bdr = packet.backup_designated_router;
    neighbor.priority = packet.router_priority;
    // 每个 Hello 包引起邻居状态机执行事件 HelloReceived
    src.hello_receive().await;
    // 如果路由器自身出现在列表中，邻居状态机执行事件 2-WayReceived
    // 否则，邻居状态机执行事件 1-WayReceived，并终止包处理过程
    if packet.neighbors.contains(&ProtocolDB::get_router_id()) {
        src.two_way_received().await;
    } else {
        src.one_way_received().await;
        return;
    }
    // 如果发现邻居优先级有改变，接收接口状态机调度执行事件 NeighborChange
    if packet.router_priority != prev_state.priority {
        iface.neighbor_change().await;
    }
    if packet.designated_router == prev_state.ip_addr
        && packet.backup_designated_router == hex2ip(0)
        && iface.state == InterfaceState::Waiting
    {
        // 如果邻居宣告自己为 DR && 而且接收接口状态机的状态为 Waiting
        // 接收接口状态机调度执行事件 BackupSeen
        iface.backup_seen().await;
    } else if packet.designated_router == prev_state.ip_addr && !prev_state.is_dr()
        || prev_state.is_dr() && packet.designated_router != prev_state.ip_addr
    {
        // 如果以前不宣告的邻居宣告自己为 DR，或以前宣告的邻居现在不宣告自己为 DR
        // 接收接口状态机调度执行事件 NeighborChange
        iface.neighbor_change().await;
    }
    if packet.backup_designated_router == prev_state.ip_addr
        && iface.state == InterfaceState::Waiting
    {
        // 如果邻居宣告自己为 BDR && 而且接收接口状态机的状态为 Waiting
        // 接收接口状态机调度执行事件 BackupSeen
        iface.backup_seen().await;
    } else if packet.backup_designated_router == prev_state.ip_addr && !prev_state.is_bdr()
        || prev_state.is_bdr() && packet.backup_designated_router != prev_state.ip_addr
    {
        // 如果以前不宣告的邻居宣告自己为 DR，或以前宣告的邻居现在不宣告自己为 DR
        // 接收接口状态机调度执行事件 NeighborChange
        iface.neighbor_change().await;
    }
}
