mod listen;
mod state;
pub use state::*;

use crate::{
    neighbor::{Neighbor, NeighborState},
    util::{do_nothing_handle, hex2ip},
};

use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{Arc, Weak},
};

use pnet::{
    datalink::{self, NetworkInterface},
    ipnetwork::IpNetwork,
    packet::ip::IpNextHeaderProtocols::OspfigP,
    transport::{
        transport_channel, TransportChannelType::Layer4, TransportProtocol::Ipv4, TransportSender,
    },
};
use tokio::sync::Mutex;

pub struct Interface {
    pub me: WInterface,
    pub interface_name: String,
    pub sender: TransportSender,
    pub net_type: NetType,
    pub state: InterfaceState,
    pub ip_addr: Ipv4Addr,
    pub ip_mask: Ipv4Addr,
    pub area_id: Ipv4Addr,
    pub hello_interval: u16,
    pub dead_interval: u32,
    pub inf_trans_delay: u16,
    pub router_priority: u8,
    pub external_routing: bool,
    pub hello_timer: tokio::task::AbortHandle,
    pub wait_timer: tokio::task::AbortHandle,
    #[doc = "ip -> neighbor (p2p|virtual => ip := router_id)"]
    pub neighbors: HashMap<Ipv4Addr, Neighbor>,
    pub dr: Ipv4Addr,
    pub bdr: Ipv4Addr,
    pub cost: u16,
    pub rxmt_interval: u16,
    pub au_type: u16,
    pub au_key: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetType {
    P2P,
    Broadcast,
    NBMA,
    P2MP,
    Virtual,
}

pub type AInterface = Arc<Mutex<Interface>>;
pub type WInterface = Weak<Mutex<Interface>>;

impl Interface {
    pub fn new(
        area_id: Ipv4Addr,
        interface_name: String,
        sender: TransportSender,
        ip_addr: Ipv4Addr,
        ip_mask: Ipv4Addr,
    ) -> AInterface {
        Arc::new_cyclic(|me| {
            Mutex::new(Self {
                me: me.clone(),
                interface_name,
                sender,
                net_type: NetType::Broadcast,
                state: InterfaceState::Down,
                ip_addr,
                ip_mask,
                area_id,
                hello_interval: 10,
                dead_interval: 40,
                inf_trans_delay: 1,
                router_priority: 1,
                external_routing: true,
                hello_timer: do_nothing_handle(),
                wait_timer: do_nothing_handle(),
                neighbors: HashMap::new(),
                dr: hex2ip(0),
                bdr: hex2ip(0),
                cost: 0,
                rxmt_interval: 4,
                au_type: 0,
                au_key: 0,
            })
        })
    }

    pub fn from(iface: &NetworkInterface, area_id: Ipv4Addr) -> AInterface {
        let ip = iface
            .ips
            .iter()
            .find_map(|ip| {
                if let IpNetwork::V4(ip) = ip {
                    Some(ip)
                } else {
                    None
                }
            })
            .expect("No IPv4 address found on interface");
        let tx = match transport_channel(4096, Layer4(Ipv4(OspfigP))) {
            Ok((tx, ..)) => tx,
            Err(e) => panic!(
                "An error occurred when creating the transport channel: {}",
                e
            ),
        };
        Self::new(area_id, iface.name.to_string(), tx, ip.ip(), ip.mask())
    }

    pub fn start(this: &AInterface) {
        listen::listen_interface(Arc::downgrade(this));
    }

    pub fn get_network_interface(&self) -> NetworkInterface {
        datalink::interfaces()
            .into_iter()
            .filter(|i| i.name == self.interface_name)
            .next()
            .expect(&format!(
                "There is no interface named {}",
                self.interface_name
            ))
    }

    pub fn shrink_neighbors(&mut self) {
        self.neighbors.retain(|_, n| n.state != NeighborState::Down);
    }

    pub fn reset(&mut self) {
        self.hello_timer.abort();
        self.wait_timer.abort();
        self.dr = hex2ip(0);
        self.bdr = hex2ip(0);
        self.cost = 0;
    }

    pub fn is_dr(&self) -> bool {
        self.dr == self.ip_addr
    }

    pub fn is_bdr(&self) -> bool {
        self.bdr == self.ip_addr
    }

    pub fn is_drother(&self) -> bool {
        !self.is_dr() && !self.is_bdr()
    }
}
