mod routing;
mod vlink;
pub use routing::RoutingTable;
pub use vlink::VirtualLink;

use std::{collections::HashMap, net::Ipv4Addr, sync::OnceLock};

pub use ospf_packet::lsa::LsaIndex;
use ospf_packet::lsa::{Lsa, LsaHeader};
use tokio::sync::Mutex;

use crate::{
    area::{Area, BackboneDB},
    guard,
};

pub struct ProtocolDB {
    pub router_id: Ipv4Addr,
    pub areas: Mutex<HashMap<Ipv4Addr, Area>>,
    pub backbone: BackboneDB,
    pub virtual_links: Mutex<Vec<VirtualLink>>,
    pub external_routes: Mutex<Vec<Ipv4Addr>>,
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

    pub async fn get_lsa(&self, area_id: Ipv4Addr, key: LsaIndex) -> Option<Lsa> {
        let lock = self.areas.lock().await;
        lock.get(&area_id)?.get_lsa(key).await
    }

    pub async fn insert_lsa(&self, area_id: Ipv4Addr, lsa: Lsa) -> bool {
        let mut lock = self.areas.lock().await;
        lock.get_mut(&area_id).unwrap().insert_lsa(lsa).await
    }

    pub async fn need_update(&self, area_id: Ipv4Addr, lsa: LsaHeader) -> bool {
        guard!(Some(me) = self.get_lsa(area_id, lsa.into()).await; ret: true);
        lsa > me.header
    }
}
