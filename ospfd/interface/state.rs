use std::{ops::DerefMut, time::Duration};

use futures::FutureExt as _;
use ospf_packet::packet;
use tokio::time::sleep;

use super::{AInterface, Interface, NetType};
use crate::{constant::AllSPFRouters, sender::send_packet};

#[cfg(debug_assertions)]
use std::ops::Deref;
#[cfg(debug_assertions)]
use crate::{log, log_success};

#[allow(unused)]
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
#[allow(unused)]
pub trait InterfaceEvent: Send {
    async fn interface_up(self);
    async fn wait_timer(self);
    async fn backup_seen(self);
    async fn neighbor_change(self);
    async fn loop_ind(self);
    async fn unloop_ind(self);
    async fn interface_down(self);
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

#[cfg(debug_assertions)]
fn log_state(old: InterfaceState, interface: &Interface) {
    log_success!(
        "interface {}'s state changed: {:?} -> {:?}",
        interface.interface_name,
        old,
        interface.state
    );
}

impl InterfaceEvent for AInterface {
    async fn interface_up(self) {
        #[cfg(debug_assertions)]
        log_event("interface_up", self.read().await.deref());
        if self.read().await.state != InterfaceState::Down {
            return;
        }
        let mut interface = self.write().await;
        let iface = interface.get_network_interface();
        interface.net_type = if iface.is_point_to_point() && iface.is_multicast() {
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
        if matches!(
            interface.net_type,
            NetType::P2P | NetType::P2MP | NetType::Virtual
        ) {
            interface.state = InterfaceState::PointToPoint;
        } else {
            interface.state = InterfaceState::Waiting;
        }
        tokio::spawn(send_hello(self.clone()));
        #[cfg(debug_assertions)]
        log_state(InterfaceState::Down, interface.deref());
    }

    async fn wait_timer(self) {
        let interface = self.read().await;
        #[cfg(debug_assertions)]
        log_event("wait_timer", interface.deref());
        if interface.state != InterfaceState::Waiting {
            return;
        }
        select_dr(self.clone()).await;
        #[cfg(debug_assertions)]
        log_state(InterfaceState::Waiting, interface.deref());
    }

    async fn backup_seen(self) {
        let interface = self.read().await;
        #[cfg(debug_assertions)]
        log_event("backup_seen", interface.deref());
        if interface.state != InterfaceState::Waiting {
            return;
        }
        select_dr(self.clone()).await;
        #[cfg(debug_assertions)]
        log_state(InterfaceState::Waiting, interface.deref());
    }

    async fn neighbor_change(self) {
        let interface = self.read().await;
        #[cfg(debug_assertions)]
        log_event("neighbor_change", interface.deref());
        let old = interface.state;
        if !matches!(
            old,
            InterfaceState::DROther | InterfaceState::Backup | InterfaceState::DR
        ) {
            return;
        }
        select_dr(self.clone()).await;
        #[cfg(debug_assertions)]
        log_state(old, interface.deref());
    }

    async fn loop_ind(self) {
        let mut interface = self.write().await;
        #[cfg(debug_assertions)]
        log_event("neighbor_change", interface.deref());
        #[cfg(debug_assertions)]
        let old = interface.state;
        interface.reset();
        interface.state = InterfaceState::Loopback;
        #[cfg(debug_assertions)]
        log_state(old, interface.deref());
    }

    async fn unloop_ind(self) {
        let mut interface = self.write().await;
        #[cfg(debug_assertions)]
        log_event("unloop_ind", interface.deref());
        let old = interface.state;
        if old != InterfaceState::Loopback {
            return;
        }
        interface.state = InterfaceState::Down;
        #[cfg(debug_assertions)]
        log_state(old, interface.deref());
    }

    async fn interface_down(self) {
        let mut interface = self.write().await;
        #[cfg(debug_assertions)]
        log_event("interface_down", interface.deref());
        #[cfg(debug_assertions)]
        let old = interface.state;
        interface.reset();
        interface.state = InterfaceState::Down;
        #[cfg(debug_assertions)]
        log_state(old, interface.deref());
    }
}

fn set_hello_timer(ifw: &mut Interface, interface: AInterface) {
    let interval = ifw.hello_interval as u64;
    ifw.hello_timer = Some(tokio::spawn(
        sleep(Duration::from_secs(interval)).then(|_| async {
            send_hello(interface).await;
        }),
    ));
}

async fn send_hello(interface: AInterface) {
    // first: set timer for next hello packet
    set_hello_timer(interface.write().await.deref_mut(), interface.clone());
    // second: send hello packet
    let ifr = interface.read().await;
    let packet = packet::HelloPacket {
        network_mask: ifr.ip_mask,
        hello_interval: ifr.hello_interval,
        options: packet::options::E,
        router_priority: ifr.router_priority,
        router_dead_interval: ifr.hello_interval as u32 * 4,
        designated_router: ifr.dr,
        backup_designated_router: ifr.bdr,
        neighbors: ifr.neighbors.clone(),
    };
    drop(ifr);
    send_packet(interface, &packet, AllSPFRouters).await;
}

//todo implement select_dr
async fn select_dr(_interface: AInterface) {
    todo!()
}
