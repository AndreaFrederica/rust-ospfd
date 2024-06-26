mod pair;
mod state;
use pair::LsrHandle;
pub use pair::RefNeighbor;
pub use state::*;

use std::{
    collections::{HashSet, VecDeque},
    net::Ipv4Addr,
};

use ospf_packet::{lsa::LsaHeader, packet::DBDescription};

use crate::{
    database::LsaIndex,
    util::{hex2ip, AbortHandle},
};

#[derive(Debug)]
pub struct Neighbor {
    pub state: NeighborState,
    pub inactive_timer: AbortHandle,
    /// true 表示邻居是 master
    pub master: bool,
    pub dd_seq_num: u32,
    /// 邻居发给自己的上一个 DD 包
    pub dd_last_packet: DdPacketCache,
    pub router_id: Ipv4Addr,
    pub priority: u8,
    pub ip_addr: Ipv4Addr,
    pub option: u8,
    pub dr: Ipv4Addr,
    pub bdr: Ipv4Addr,
    /// DD 包重传 （仅主机才能重传）
    pub dd_rxmt: DdRxmt,
    /// lsr sender
    pub lsr_handle: LsrHandle,
    /// 已经被洪泛，但还没有从邻接得到确认的 LSA 列表 (等待 LS ACK)
    pub ls_retransmission_list: HashSet<LsaIndex>,
    /// 区域连接状态数据库中 LSA 的完整列表 (发送 DD 时需要附带的)
    pub db_summary_list: VecDeque<LsaHeader>,
    /// 需要从邻居接收，以同步两者之间连接状态数据库的 LSA 列表 （需要发送 LSR）
    pub ls_request_list: VecDeque<LsaHeader>,
}

impl Neighbor {
    pub fn new(router_id: Ipv4Addr, ip_addr: Ipv4Addr) -> Neighbor {
        Self {
            state: NeighborState::Down,
            inactive_timer: AbortHandle::default(),
            master: false,
            dd_seq_num: 0,
            dd_last_packet: DdPacketCache::default(),
            router_id,
            priority: 0,
            ip_addr,
            option: 0,
            dr: hex2ip(0),
            bdr: hex2ip(0),
            dd_rxmt: DdRxmt::None,
            lsr_handle: LsrHandle::default(),
            ls_retransmission_list: HashSet::new(),
            db_summary_list: VecDeque::new(),
            ls_request_list: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.dd_seq_num = 0;
        self.dd_rxmt.reset();
        self.lsr_handle.abort();
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

#[derive(Debug)]
pub enum DdRxmt {
    Handle(AbortHandle),
    Packet(DBDescription),
    None,
}

impl DdRxmt {
    pub fn reset(&mut self) {
        *self = DdRxmt::None;
    }

    pub fn set(&mut self, other: DdRxmt) {
        self.reset();
        *self = other;
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

impl std::fmt::Display for Neighbor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Router ID: {}\t\tAddress: {}", self.router_id, self.ip_addr)?;
        writeln!(f, "  State: {:?}\tMode: {}\tPriority: {}", self.state, if self.master { "master" } else { "slave" }, self.priority)?;
        writeln!(f, "  DR: {}\t\tBDR: {}", self.dr, self.bdr)?;
        Ok(())
    }
}
