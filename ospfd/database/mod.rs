mod routing;
mod vlink;
pub use routing::RoutingTable;
pub use vlink::VirtualLink;

use std::{collections::HashMap, net::Ipv4Addr, sync::OnceLock};

use lazy_static::lazy_static;
pub use ospf_packet::lsa::LsaIndex;
use ospf_packet::lsa::{Lsa, LsaHeader};
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    area::{Area, BackboneDB},
    guard,
    interface::{AInterface, Interface},
};

static ROUTER_ID: OnceLock<Ipv4Addr> = OnceLock::new();
static INTERFACES: OnceLock<Vec<AInterface>> = OnceLock::new();

pub struct ProtocolDB {
    pub areas: HashMap<Ipv4Addr, Area>,
    pub backbone: BackboneDB,
    pub virtual_links: Vec<VirtualLink>,
    pub external_routes: Vec<Ipv4Addr>,
    pub routing_table: RoutingTable,
}

lazy_static! {
    static ref DATABASE: Mutex<ProtocolDB> = Mutex::new(ProtocolDB {
        areas: HashMap::new(),
        backbone: BackboneDB::new(),
        virtual_links: Vec::new(),
        external_routes: Vec::new(),
        routing_table: RoutingTable::new(),
    });
}

type Guard<T> = MutexGuard<'static, T>;

/// # Safety
/// 锁的使用要求：
/// 只有 INTERFACES 和 DATABASE 有锁。
/// 1. DATABASE 的锁必须在持有任何 INTERFACE 的锁时才能获取。
/// 2. 任何异步调用，要么持有一把 INTERFACE 的锁，要么通过 get_interfaces 获取所有 INTERFACE 的锁。
/// 3. 在已经持有一把 INTERFACE 的锁的情况下，可以通过 upgrade_lock 升级锁。
impl ProtocolDB {
    pub fn init(interfaces: &Vec<AInterface>) {
        use tokio::task::block_in_place;
        INTERFACES.get_or_init(|| interfaces.clone());
        ROUTER_ID.get_or_init(|| {
            interfaces
                .iter()
                .map(|i| block_in_place(|| i.blocking_lock().ip_addr))
                .min()
                .unwrap()
        });
    }

    pub fn get_router_id() -> Ipv4Addr {
        *ROUTER_ID.get().unwrap()
    }

    /// # Safety
    /// This function should be awaited when caller hasn't have any locks.
    pub async fn get_interfaces() -> Vec<Guard<Interface>> {
        let rt = tokio::runtime::Handle::current();
        INTERFACES
            .get()
            .unwrap()
            .iter()
            .map(|iface| rt.block_on(iface.lock()))
            .collect()
    }

    pub async fn upgrade_lock(iface: Guard<Interface>) -> InterfacesGuard {
        let ip = iface.ip_addr;
        drop(iface);
        let interfaces = Self::get_interfaces().await;
        InterfacesGuard::from(interfaces, ip)
    }

    /// # Safety
    /// This function should be awaited when caller have an interface's lock or all locks.
    pub async fn get() -> Guard<ProtocolDB> {
        DATABASE.lock().await
    }

    pub async fn insert_area(&mut self, area_id: Ipv4Addr) {
        if !self.areas.contains_key(&area_id) {
            self.areas.insert(area_id, Area::new(area_id));
        }
    }

    pub async fn get_lsa(&self, area_id: Ipv4Addr, key: LsaIndex) -> Option<Lsa> {
        self.areas.get(&area_id)?.get_lsa(key).await
    }

    pub async fn insert_lsa(&mut self, area_id: Ipv4Addr, lsa: Lsa) -> bool {
        self.areas.get_mut(&area_id).unwrap().insert_lsa(lsa).await
    }

    pub async fn need_update(&self, area_id: Ipv4Addr, lsa: LsaHeader) -> bool {
        guard!(Some(me) = self.get_lsa(area_id, lsa.into()).await; ret: true);
        lsa > me.header
    }
}

pub struct InterfacesGuard {
    pub me: Guard<Interface>,
    pub other: Vec<Guard<Interface>>,
}

impl InterfacesGuard {
    fn from(mut vec: Vec<MutexGuard<'static, Interface>>, ip: Ipv4Addr) -> Self {
        let me = vec.swap_remove(vec.iter().position(|i| i.ip_addr == ip).unwrap());
        Self { me, other: vec }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Guard<Interface>> {
        std::iter::once(&self.me).chain(self.other.iter())
    }
}

impl IntoIterator for InterfacesGuard {
    type Item = Guard<Interface>;
    type IntoIter = std::iter::Chain<std::iter::Once<Self::Item>, std::vec::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.me).chain(self.other.into_iter())
    }
}
