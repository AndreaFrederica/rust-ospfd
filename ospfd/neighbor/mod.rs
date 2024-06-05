mod state;
use std::{collections::HashMap, net::Ipv4Addr, sync::Arc};

pub use state::*;

use lazy_static::lazy_static;
use ospf_packet::lsa::Lsa;
use tokio::sync::RwLock;

use crate::util::hex2ip;

use super::types::*;

#[derive(Debug)]
pub struct Neighbor {
    pub state: NeighborState,
    pub inactive_timer: TimerHandle,
    pub master: bool,
    pub dd_seq_num: u32,
    pub dd_last_packet: u32,
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
type NeighborMap = RwLock<HashMap<Ipv4Addr, ANeighbor>>;

lazy_static! {
    pub static ref ID2NEIGHBORS: NeighborMap = RwLock::new(HashMap::new());
}

impl Neighbor {
    pub fn new(router_id: Ipv4Addr, ip_addr: Ipv4Addr) -> ANeighbor {
        Arc::new(RwLock::new(Self {
            state: NeighborState::Down,
            inactive_timer: None,
            master: false,
            dd_seq_num: 0,
            dd_last_packet: 0,
            router_id,
            priority: 0,
            ip_addr,
            option: 0,
            dr: hex2ip(0),
            bdr: hex2ip(0),
            ls_retransmission_list: Vec::new(),
            db_summary_list: Vec::new(),
            ls_request_list: Vec::new(),
        }))
    }

    pub async fn get_by_id(router_id: Ipv4Addr) -> Option<ANeighbor> {
        return ID2NEIGHBORS.read().await.get(&router_id).cloned();
    }
}
