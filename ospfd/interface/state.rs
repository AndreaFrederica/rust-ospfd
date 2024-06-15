use std::{
    net::Ipv4Addr,
    ops::{Deref, DerefMut},
    time::Duration,
};

use ospf_packet::packet::{self, options::OptionExt};
use tokio::time::sleep;

use super::{Interface, NetType};
use crate::{
    constant::AllSPFRouters,
    database::ProtocolDB,
    guard, log_error, log_success, must,
    neighbor::{Neighbor, NeighborEvent, NeighborState, RefNeighbor},
    sender::send_packet,
    util::hex2ip,
};

#[cfg(debug_assertions)]
use crate::log;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceState {
    Down,
    Loopback,
    Waiting,
    PointToPoint,
    DROther,
    Backup,
    DR,
}

// helper trait for event handling
pub trait InterfaceEvent: Send {
    async fn interface_up(&mut self);
    async fn wait_timer(&mut self);
    async fn backup_seen(&mut self);
    async fn neighbor_change(&mut self);
    async fn loop_ind(&mut self);
    async fn unloop_ind(&mut self);
    async fn interface_down(&mut self);
}

#[cfg(debug_assertions)]
fn log_event(event: &str, interface: &Interface) {
    log!(
        "interface {}({:?}) recv event: {}",
        interface.interface_name,
        interface.state,
        event
    );
}

fn log_state(old: InterfaceState, interface: &Interface) {
    log_success!(
        "interface {}'s state changed: {:?} -> {:?}",
        interface.interface_name,
        old,
        interface.state
    );
}

impl InterfaceEvent for Interface {
    async fn interface_up(&mut self) {
        #[cfg(debug_assertions)]
        log_event("interface_up", self);
        must!(self.state == InterfaceState::Down);
        let iface = self.get_network_interface();
        self.net_type = if iface.is_point_to_point() && iface.is_multicast() {
            NetType::P2MP
        } else if iface.is_point_to_point() {
            NetType::P2P
        } else if iface.is_broadcast() {
            NetType::Broadcast
        } else if iface.is_multicast() {
            NetType::NBMA
        } else {
            NetType::Virtual
        };
        self.state = if matches!(
            self.net_type,
            NetType::P2P | NetType::P2MP | NetType::Virtual
        ) {
            InterfaceState::PointToPoint
        } else if self.router_priority == 0 {
            InterfaceState::DROther
        } else {
            let weak = self.me.clone();
            self.wait_timer = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(20)).await;
                guard!(Some(iface) = weak.upgrade());
                iface.lock().await.wait_timer().await;
            })
            .abort_handle();
            InterfaceState::Waiting
        };
        set_hello_timer(self);
        log_state(InterfaceState::Down, self);
    }

    async fn wait_timer(&mut self) {
        #[cfg(debug_assertions)]
        log_event("wait_timer", self);
        must!(self.state == InterfaceState::Waiting);
        select_dr(self).await;
        log_state(InterfaceState::Waiting, self);
    }

    async fn backup_seen(&mut self) {
        #[cfg(debug_assertions)]
        log_event("backup_seen", self);
        must!(self.state == InterfaceState::Waiting);
        select_dr(self).await;
        log_state(InterfaceState::Waiting, self);
    }

    async fn neighbor_change(&mut self) {
        #[cfg(debug_assertions)]
        log_event("neighbor_change", self);
        let old = self.state;
        use InterfaceState::*;
        must!(matches!(old, DROther | Backup | DR));
        select_dr(self).await;
        log_state(old, self);
    }

    async fn loop_ind(&mut self) {
        #[cfg(debug_assertions)]
        log_event("neighbor_change", self);
        let old = self.state;
        self.reset();
        self.state = InterfaceState::Loopback;
        log_state(old, self);
    }

    async fn unloop_ind(&mut self) {
        #[cfg(debug_assertions)]
        log_event("unloop_ind", self);
        let old = self.state;
        must!(old == InterfaceState::Loopback);
        self.state = InterfaceState::Down;
        log_state(old, self);
    }

    async fn interface_down(&mut self) {
        #[cfg(debug_assertions)]
        log_event("interface_down", self);
        let old = self.state;
        self.reset();
        self.state = InterfaceState::Down;
        log_state(old, self);
    }
}

fn set_hello_timer(interface: &mut Interface) {
    let weak = interface.me.clone();
    interface.hello_timer.abort();
    interface.hello_timer = tokio::spawn(async move {
        while let Some(interface) = weak.upgrade() {
            let mut interface = interface.lock().await;
            let hello_interval = interface.hello_interval as u64;
            send_hello(interface.deref_mut()).await;
            drop(interface); // drop here to avoid sleep with a lock...
            sleep(Duration::from_secs(hello_interval)).await;
        }
        crate::log_warning!("interface is dropped, hello timer stopped");
    })
    .abort_handle();
}

async fn send_hello(interface: &mut Interface) {
    // first: shrink neighbors
    interface.shrink_neighbors();
    // second: send hello packet
    let mut packet = packet::HelloPacket {
        network_mask: interface.ip_mask,
        hello_interval: interface.hello_interval,
        options: 0,
        router_priority: interface.router_priority,
        router_dead_interval: interface.dead_interval,
        designated_router: interface.dr,
        backup_designated_router: interface.bdr,
        neighbors: interface.neighbors.values().map(|n| n.router_id).collect(),
    };
    if interface.external_routing {
        packet.set(packet::options::E);
    }
    send_packet(interface, &packet, AllSPFRouters).await;
}

#[derive(Debug, Clone, Copy)]
struct SelectDr {
    priority: u8,
    id: Ipv4Addr,
    ip: Ipv4Addr,
    bdr: Ipv4Addr,
    dr: Ipv4Addr,
}

impl From<&Neighbor> for SelectDr {
    fn from(value: &Neighbor) -> Self {
        SelectDr {
            priority: value.priority,
            id: value.router_id,
            ip: value.ip_addr,
            bdr: value.bdr,
            dr: value.dr,
        }
    }
}

impl From<&Interface> for SelectDr {
    fn from(value: &Interface) -> Self {
        SelectDr {
            priority: value.router_priority,
            id: ProtocolDB::get_router_id(),
            ip: value.ip_addr,
            bdr: value.bdr,
            dr: value.dr,
        }
    }
}

async fn select_dr(interface: &mut Interface) {
    // step1: find all available neighbors
    let mut can: Vec<SelectDr> = interface
        .neighbors
        .values()
        .filter(|n| n.state >= NeighborState::TwoWay)
        .map(|n| n.into())
        .collect();
    can.push(interface.deref().into());
    can = can.into_iter().filter(|v| v.priority > 0).collect();
    let cmp = |x: &&SelectDr, y: &&SelectDr| {
        if x.priority == y.priority {
            x.id.cmp(&y.id)
        } else {
            x.priority.cmp(&y.priority)
        }
    };
    loop {
        // step2: select bdr
        let bdr = {
            let can: Vec<_> = can.iter().filter(|v| v.dr != v.ip).copied().collect();
            let vec: Vec<_> = can.iter().filter(|v| v.bdr == v.ip).copied().collect();
            if vec.is_empty() { can } else { vec }
                .iter()
                .max_by(cmp)
                .map(|v| v.ip)
                .unwrap_or(hex2ip(0))
        };
        // step3: select dr
        let dr = {
            let vec: Vec<_> = can.iter().filter(|v| v.dr == v.ip).copied().collect();
            vec.iter().max_by(cmp).map(|v| v.ip).unwrap_or(bdr)
        };
        let bdr = if dr == bdr { hex2ip(0) } else { bdr };
        // step4: state change
        let new_select = dr == interface.ip_addr && !interface.is_dr()
            || bdr == interface.ip_addr && !interface.is_bdr()
            || interface.is_dr() && dr != interface.ip_addr
            || interface.is_bdr() && bdr != interface.ip_addr;
        interface.dr = dr;
        interface.bdr = bdr;
        if !new_select {
            break;
        }
    }
    // step5: state change
    interface.state = if interface.is_dr() {
        InterfaceState::DR
    } else if interface.is_bdr() {
        InterfaceState::Backup
    } else {
        InterfaceState::DROther
    };
    // step6: send hello packet
    if interface.net_type == NetType::NBMA {
        log_error!("NBMA not implemented");
    }
    // step7: AdjOk?
    let keys: Vec<_> = interface.neighbors.keys().cloned().collect();
    for ip in keys {
        RefNeighbor::from(interface, ip).unwrap().adj_ok().await;
    }
}
