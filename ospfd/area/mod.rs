mod backbone;
mod tree;
pub use backbone::BackboneDB;
pub use tree::ShortPathTree;

use std::{collections::BTreeMap, net::Ipv4Addr};

use ospf_packet::lsa::{LsaHeader, NetworkLSA, RouterLSA, SummaryLSA};

#[derive(Debug)]
pub struct Area {
    pub area_id: Ipv4Addr,
    /// ［地址、掩码］-> 宣告状态
    pub addr_range: BTreeMap<(Ipv4Addr, Ipv4Addr), bool>,
    pub router_lsa: Vec<(LsaHeader, RouterLSA)>,
    pub network_lsa: Vec<(LsaHeader, NetworkLSA)>,
    pub summary_lsa: Vec<(LsaHeader, SummaryLSA)>,
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
            router_lsa: Vec::new(),
            network_lsa: Vec::new(),
            summary_lsa: Vec::new(),
            short_path_tree: ShortPathTree::new(),
            transit_capability: false,
            external_routing_capability: true,
            stub_default_cost: 0,
        }
    }
}
