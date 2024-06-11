mod routing;
mod vlink;
pub use routing::RoutingTable;
pub use vlink::VirtualLink;

use std::{collections::HashMap, net::Ipv4Addr, sync::OnceLock};

use ospf_packet::lsa::{self, AsExternalLSA, Lsa, LsaHeader};
use tokio::sync::Mutex;

use crate::{
    area::{Area, BackboneDB},
    guard,
};

type LsaAsExternal = (LsaHeader, AsExternalLSA);

#[derive(Debug)]
pub struct ProtocolDB {
    pub router_id: Ipv4Addr,
    pub areas: Mutex<HashMap<Ipv4Addr, Area>>,
    pub backbone: BackboneDB,
    pub virtual_links: Mutex<Vec<VirtualLink>>,
    pub external_routes: Mutex<Vec<Ipv4Addr>>,
    pub as_external_lsa: Mutex<HashMap<LsaMapIndex, LsaAsExternal>>,
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
            as_external_lsa: Mutex::new(HashMap::new()),
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

    pub async fn get_lsa(&self, area_id: Ipv4Addr, index: LsaIndex) -> Option<Lsa> {
        let lock = self.areas.lock().await;
        let area = lock.get(&area_id)?;
        let as_external = self.as_external_lsa.lock().await;
        match index.ls_type {
            lsa::types::ROUTER_LSA => area
                .router_lsa
                .get(&index.into())
                .map(|v| v.clone().try_into().unwrap()),
            lsa::types::NETWORK_LSA => area
                .network_lsa
                .get(&index.into())
                .map(|v| v.clone().try_into().unwrap()),
            lsa::types::SUMMARY_IP_LSA => area
                .ip_summary_lsa
                .get(&index.into())
                .map(|v| v.clone().try_into().unwrap()),
            lsa::types::SUMMARY_ASBR_LSA => area
                .asbr_summary_lsa
                .get(&index.into())
                .map(|v| v.clone().try_into().unwrap()),
            lsa::types::AS_EXTERNAL_LSA => as_external
                .get(&index.into())
                .map(|v| v.clone().try_into().unwrap()),
            _ => None,
        }
    }

    pub async fn need_update(&self, area_id: Ipv4Addr, lsa: &LsaHeader) -> bool {
        guard!(Some(me) = self.get_lsa(area_id, lsa.into()).await; ret: true);
        lsa > &me.header
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LsaIndex {
    pub ls_type: u8,
    pub ls_id: u32,
    pub ad_router: Ipv4Addr,
}

impl LsaIndex {
    pub fn new(ls_type: u8, ls_id: u32, ad_router: Ipv4Addr) -> Self {
        Self {
            ls_type,
            ls_id,
            ad_router,
        }
    }
}

impl From<&LsaHeader> for LsaIndex {
    fn from(value: &LsaHeader) -> Self {
        Self {
            ls_type: value.ls_type,
            ls_id: value.link_state_id,
            ad_router: value.advertising_router,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LsaMapIndex {
    pub ls_id: u32,
    pub ad_router: Ipv4Addr,
}

impl From<LsaIndex> for LsaMapIndex {
    fn from(value: LsaIndex) -> Self {
        Self {
            ls_id: value.ls_id,
            ad_router: value.ad_router,
        }
    }
}
