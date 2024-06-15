mod backbone;
mod lsa;
mod tree;
pub use backbone::BackboneDB;
pub use tree::ShortPathTree;

use std::{
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
    time::Instant,
};

use lazy_static::lazy_static;
use ospf_packet::lsa::{types::AS_EXTERNAL_LSA, Lsa, LsaHeader, LsaIndex};
use tokio::sync::Mutex;

use lsa::LsaTimer;

use crate::{constant::LsaMaxAge, database::ProtocolDB, guard};

/// (lsa, created_at, updated_at)
type LsaDB = HashMap<LsaIndex, (Lsa, LsaTimer, Instant)>;

lazy_static! {
    /// AS External LSA database
    static ref STATIC_DB: Mutex<LsaDB> =
        Mutex::const_new(HashMap::new());
}

pub struct Area {
    pub area_id: Ipv4Addr,
    /// ［地址、掩码］-> 宣告状态
    pub addr_range: BTreeMap<(Ipv4Addr, Ipv4Addr), bool>,
    lsa_database: LsaDB,
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
            lsa_database: LsaDB::new(),
            short_path_tree: ShortPathTree::new(),
            transit_capability: false,
            external_routing_capability: true,
            stub_default_cost: 0,
        }
    }
}

impl Area {
    fn m_external_db<T>(&self, db: T) -> Option<T> {
        if self.external_routing_capability {
            Some(db)
        } else {
            None
        }
    }

    fn m_get_lsa(&self, db: &LsaDB, key: LsaIndex) -> Option<(Lsa, Instant, Instant)> {
        self.lsa_database
            .get(&key)
            .or(self.m_external_db(db)?.get(&key))
            .map(|(lsa, timer, up)| (timer.update_lsa_age(lsa.clone()), timer.get_created(), *up))
    }

    fn m_insert_lsa(&mut self, db: &mut LsaDB, key: LsaIndex, value: Lsa) {
        let t = (LsaMaxAge - value.header.ls_age) as u64;
        let timer = LsaTimer::new(t, refresh_lsa(self.area_id, value.clone()));
        if self.external_routing_capability && matches!(key.ls_type, AS_EXTERNAL_LSA) {
            db.insert(key, (value, timer, Instant::now()));
        } else {
            assert!(!matches!(key.ls_type, AS_EXTERNAL_LSA));
            self.lsa_database
                .insert(key, (value, timer, Instant::now()));
        }
    }

    fn m_remove_lsa(&mut self, db: &mut LsaDB, key: LsaIndex) {
        self.lsa_database
            .remove(&key)
            .or_else(|| self.m_external_db(db)?.remove(&key));
    }

    pub async fn contains_lsa(&self, key: LsaIndex) -> bool {
        let db = STATIC_DB.lock().await;
        self.lsa_database.contains_key(&key)
            || self.external_routing_capability && db.contains_key(&key)
    }

    pub async fn get_lsa(&self, key: LsaIndex) -> Option<(Lsa, Instant, Instant)> {
        let db = STATIC_DB.lock().await;
        self.m_get_lsa(&db, key)
    }

    pub async fn get_all_lsa(&self) -> Vec<LsaHeader> {
        let my = self
            .lsa_database
            .values()
            .map(|(lsa, timer, _)| timer.update_lsa_age_header(lsa.header.clone()))
            .filter(|header| header.ls_age != LsaMaxAge);
        if self.external_routing_capability {
            let db = STATIC_DB.lock().await;
            my.chain(
                db.values()
                    .map(|(lsa, timer, _)| timer.update_lsa_age_header(lsa.header.clone()))
                    .filter(|header| header.ls_age != LsaMaxAge),
            )
            .collect()
        } else {
            my.collect()
        }
    }

    pub async fn need_update(&self, header: LsaHeader) -> bool {
        let key = header.into();
        let db = STATIC_DB.lock().await;
        if let Some((lsa, ..)) = self.m_get_lsa(&db, key) {
            if lsa.header >= header {
                return false;
            }
        }
        true
    }

    pub async fn insert_lsa(&mut self, value: Lsa) {
        assert!(self.need_update(value.header).await);
        let mut db = STATIC_DB.lock().await;
        self.m_insert_lsa(&mut db, value.header.into(), value);
    }

    pub async fn remove_lsa(&mut self, key: LsaIndex) {
        let mut db = STATIC_DB.lock().await;
        self.m_remove_lsa(&mut db, key);
    }

    pub async fn lsa_has_sent(&mut self, lsa: &Lsa) {
        let key = lsa.header.into();
        let mut db = self.m_external_db(STATIC_DB.lock().await);
        let external = db.as_mut().and_then(|db| db.get_mut(&key));
        guard!(Some(db) = self.lsa_database.get_mut(&key).or(external));
        db.2 = Instant::now();
    }
}

async fn refresh_lsa(area_id: Ipv4Addr, lsa: Lsa) {
    crate::log_error!("lsa in {area_id} expired: {lsa:?}");
    let lock = &mut ProtocolDB::get().await.areas;
    let area = lock.get_mut(&area_id).unwrap();
    area.remove_lsa(lsa.header.into()).await;
    todo!()
}
