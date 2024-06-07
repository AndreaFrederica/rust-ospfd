mod routing;
mod vlink;
pub use routing::RoutingTable;
pub use vlink::VirtualLink;

use std::{collections::HashMap, net::Ipv4Addr, sync::OnceLock};

use ospf_packet::lsa::{AsExternalLSA, LsaHeader};
use tokio::sync::Mutex;

use crate::area::{Area, BackboneDB};

#[derive(Debug)]
pub struct ProtocolDB {
    pub router_id: Ipv4Addr,
    pub areas: Mutex<HashMap<Ipv4Addr, Area>>,
    pub backbone: BackboneDB,
    pub virtual_links: Mutex<Vec<VirtualLink>>,
    pub external_routes: Mutex<Vec<Ipv4Addr>>,
    pub as_external_lsa: Mutex<Vec<(LsaHeader, AsExternalLSA)>>,
    pub routing_table: RoutingTable,
}

static DATABASE: OnceLock<ProtocolDB> = OnceLock::new();

impl ProtocolDB {
    pub fn init(router_id: Ipv4Addr) {
        DATABASE.get_or_init(move || Self {
            router_id,
            areas: Mutex::new(HashMap::new()),
            backbone: BackboneDB::new(),
            virtual_links: Mutex::new(Vec::new()),
            external_routes: Mutex::new(Vec::new()),
            as_external_lsa: Mutex::new(Vec::new()),
            routing_table: RoutingTable::new(),
        });
    }

    pub fn get() -> &'static Self {
        DATABASE.get().unwrap()
    }

    pub async fn insert_area(&self, area_id: Ipv4Addr) {
        let mut lock = self.areas.lock().await;
        if !lock.contains_key(&area_id) {
            lock.insert(area_id, Area::new(area_id));
        }
    }
}
