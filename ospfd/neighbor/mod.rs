mod state;
use std::{net::Ipv4Addr, sync::Arc};

pub use state::*;

use ospf_packet::{lsa::Lsa, packet::DBDescription};
use tokio::sync::RwLock;

use crate::{interface::{AInterface, WInterface}, util::hex2ip};

use super::types::*;

#[derive(Debug)]
pub struct Neighbor {
    pub interface: WInterface,
    pub state: NeighborState,
    pub inactive_timer: TimerHandle,
    #[doc = "if the neighbor is master"]
    pub master: bool,
    pub dd_seq_num: u32,
    pub dd_last_packet: DdPacketCache,
    pub router_id: Ipv4Addr,
    pub priority: u8,
    pub ip_addr: Ipv4Addr,
    pub option: u8,
    pub dr: Ipv4Addr,
    pub bdr: Ipv4Addr,
    pub ls_retransmission_list: Vec<Lsa>,
    pub db_summary_list: Vec<Lsa>,
    pub ls_request_list: Vec<Lsa>,
}

pub type ANeighbor = Arc<RwLock<Neighbor>>;

impl Neighbor {
    pub fn new(interface: &AInterface, router_id: Ipv4Addr, ip_addr: Ipv4Addr) -> ANeighbor {
        let this = Arc::new(RwLock::new(Self {
            interface: Arc::downgrade(&interface),
            state: NeighborState::Down,
            inactive_timer: None,
            master: false,
            dd_seq_num: 0,
            dd_last_packet: DdPacketCache::default(),
            router_id,
            priority: 0,
            ip_addr,
            option: 0,
            dr: hex2ip(0),
            bdr: hex2ip(0),
            ls_retransmission_list: Vec::new(),
            db_summary_list: Vec::new(),
            ls_request_list: Vec::new(),
        }));
        this
    }

    pub fn reset(&mut self) {
        self.ls_retransmission_list.clear();
        self.db_summary_list.clear();
        self.ls_request_list.clear();
    }

    pub fn is_dr(&self) -> bool {
        self.ip_addr == self.dr
    }

    pub fn is_bdr(&self) -> bool {
        self.ip_addr == self.bdr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DdPacketCache {
    pub sequence_number: u32,
    pub init: bool,
    pub more: bool,
    pub master: bool,
}

impl From<&DBDescription> for DdPacketCache {
    fn from(value: &DBDescription) -> Self {
        Self {
            sequence_number: value.db_sequence_number,
            init: value.init != 0,
            more: value.more != 0,
            master: value.master != 0,
        }
    }
}

impl DdPacketCache {
    pub fn default() -> Self {
        Self {
            sequence_number: 0,
            init: false,
            more: false,
            master: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NeighborSubStruct {
    pub state: NeighborState,
    pub master: bool,
    pub dd_seq_num: u32,
    pub dd_last_packet: DdPacketCache,
    pub router_id: Ipv4Addr,
    pub priority: u8,
    pub ip_addr: Ipv4Addr,
    pub option: u8,
    pub dr: Ipv4Addr,
    pub bdr: Ipv4Addr,
}

impl From<&Neighbor> for NeighborSubStruct {
    fn from(value: &Neighbor) -> Self {
        Self {
            state: value.state,
            master: value.master,
            dd_seq_num: value.dd_seq_num,
            dd_last_packet: value.dd_last_packet,
            router_id: value.router_id,
            priority: value.priority,
            ip_addr: value.ip_addr,
            option: value.option,
            dr: value.dr,
            bdr: value.bdr,
        }
    }
}

impl NeighborSubStruct {
    pub fn is_dr(&self) -> bool {
        self.ip_addr == self.dr
    }

    pub fn is_bdr(&self) -> bool {
        self.ip_addr == self.bdr
    }
}
