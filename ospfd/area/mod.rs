mod backbone;
mod tree;
pub use backbone::BackboneDB;
pub use tree::ShortPathTree;

use std::{collections::{BTreeMap, HashMap}, net::Ipv4Addr};

use ospf_packet::lsa::{LsaHeader, NetworkLSA, RouterLSA, SummaryLSA};

use crate::database::LsaMapIndex;

type LsaRouter = (LsaHeader, RouterLSA);
type LsaNetwork = (LsaHeader, NetworkLSA);
type LsaSummary = (LsaHeader, SummaryLSA);

#[derive(Debug)]
pub struct Area {
    pub area_id: Ipv4Addr,
    /// ［地址、掩码］-> 宣告状态
    pub addr_range: BTreeMap<(Ipv4Addr, Ipv4Addr), bool>,
    pub router_lsa: HashMap<LsaMapIndex, LsaRouter>,
    pub network_lsa: HashMap<LsaMapIndex, LsaNetwork>,
    pub ip_summary_lsa: HashMap<LsaMapIndex, LsaSummary>,
    pub asbr_summary_lsa: HashMap<LsaMapIndex, LsaSummary>,
    pub short_path_tree: ShortPathTree,
    pub transit_capability: bool,
    pub external_routing_capability: bool,
    pub stub_default_cost: u32,
}

impl Area {
    pub fn new(area_id: Ipv4Addr) -> Self {
        Self {
            area_id,
            addr_range: BTreeMap::new(),
            router_lsa: HashMap::new(),
            network_lsa: HashMap::new(),
            ip_summary_lsa: HashMap::new(),
            asbr_summary_lsa: HashMap::new(),
            short_path_tree: ShortPathTree::new(),
            transit_capability: false,
            external_routing_capability: true,
            stub_default_cost: 0,
        }
    }
}
