mod listen;
mod state;
pub use state::*;

use crate::{
    database::ProtocolDB,
    neighbor::{ANeighbor, NeighborState},
    types::*,
    util::hex2ip,
};

use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{Arc, Weak},
};

use futures::executor;
use pnet::{
    datalink::{self, NetworkInterface},
    ipnetwork::IpNetwork,
    packet::ip::IpNextHeaderProtocols::OspfigP,
    transport::{
        transport_channel, TransportChannelType::Layer4, TransportProtocol::Ipv4, TransportSender,
    },
};
use tokio::sync::RwLock;

pub struct Interface {
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
    pub hello_timer: TimerHandle,
    pub wait_timer: TimerHandle,
    #[doc = "ip -> neighbor (p2p|virtual => ip := router_id)"]
    pub neighbors: HashMap<Ipv4Addr, ANeighbor>,
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

pub type AInterface = Arc<RwLock<Interface>>;
pub type WInterface = Weak<RwLock<Interface>>;

impl Interface {
    pub async fn new(
        area_id: Ipv4Addr,
        interface_name: String,
        sender: TransportSender,
        ip_addr: Ipv4Addr,
        ip_mask: Ipv4Addr,
    ) -> AInterface {
        let this = Arc::new(RwLock::new(Self {
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
            hello_timer: None,
            wait_timer: None,
            neighbors: HashMap::new(),
            dr: hex2ip(0),
            bdr: hex2ip(0),
            cost: 0,
            rxmt_interval: 1,
            au_type: 0,
            au_key: 0,
        }));
        ProtocolDB::get().insert_area(area_id).await;
        listen::listen_interface(Arc::downgrade(&this));
        this
    }

    pub async fn from(iface: &NetworkInterface, area_id: Ipv4Addr) -> AInterface {
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
        Self::new(area_id, iface.name.to_string(), tx, ip.ip(), ip.mask()).await
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

    pub async fn shrink_neighbors(&mut self) {
        self.neighbors
            .retain(|_, n| executor::block_on(n.read()).state != NeighborState::Down);
    }

    pub async fn get_neighbor(&self, ip: Ipv4Addr) -> Option<ANeighbor> {
        self.neighbors.get(&ip).cloned()
    }

    pub fn reset(&mut self) {
        self.hello_timer.take().map(|f| f.abort());
        self.wait_timer.take().map(|f| f.abort());
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
