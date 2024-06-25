mod routing;
mod vlink;
pub use routing::*;
pub use vlink::VirtualLink;

use std::{collections::HashMap, net::Ipv4Addr, sync::OnceLock, time::Instant};

use lazy_static::lazy_static;
pub use ospf_packet::lsa::LsaIndex;
use ospf_packet::lsa::{Lsa, LsaHeader};
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    area::{Area, BackboneDB},
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
    pub fn get_interfaces_impl() -> Vec<Guard<Interface>> {
        INTERFACES
            .get()
            .unwrap()
            .iter()
            .map(|iface| iface.blocking_lock())
            .collect()
    }

    /// # Safety
    /// This function should be awaited when caller hasn't have any locks.
    pub fn get_interface_by_name(name: &str) -> Option<Guard<Interface>> {
        Self::get_interfaces_impl()
            .into_iter()
            .find(|iface| iface.interface_name == name)
    }

    pub async fn upgrade_lock(iface: MutexGuard<'_, Interface>) -> InterfacesGuard {
        let ip = iface.ip_addr;
        drop(iface);
        let interfaces = tokio::task::block_in_place(Self::get_interfaces_impl);
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

    pub async fn recalc_routing(&mut self) {
        self.routing_table
            .recalculate(self.areas.values_mut().collect())
            .await;
    }
}

macro_rules! delegating {
    ($func:ident, $param:ty $(,$ret:ty)?) => {
        pub async fn $func(&self, area_id: Ipv4Addr, key: $param) $(-> $ret)? {
            self.areas.get(&area_id).unwrap().$func(key).await
        }
    };
    ($func:ident, mut, $param:ty $(,$ret:ty)?) => {
        pub async fn $func(&mut self, area_id: Ipv4Addr, key: $param) $(-> $ret)? {
            self.areas.get_mut(&area_id).unwrap().$func(key).await
        }
    };
}

impl ProtocolDB {
    delegating!(insert_lsa, mut, Lsa);
    delegating!(get_lsa, LsaIndex, Option<(Lsa, Instant, Instant)>);
    delegating!(contains_lsa, LsaIndex, bool);
    delegating!(need_update, LsaHeader, bool);
    delegating!(lsa_has_sent, mut, &Lsa);
}

pub struct InterfacesGuard {
    pub me: Guard<Interface>,
    pub other: Vec<Guard<Interface>>,
}

impl From<Vec<Guard<Interface>>> for InterfacesGuard {
    fn from(value: Vec<Guard<Interface>>) -> Self {
        let mut iter = value.into_iter();
        Self {
            me: iter.next().unwrap(),
            other: iter.collect(),
        }
    }
}

impl InterfacesGuard {
    fn from(mut vec: Vec<MutexGuard<'static, Interface>>, ip: Ipv4Addr) -> Self {
        let me = vec.swap_remove(vec.iter().position(|i| i.ip_addr == ip).unwrap());
        Self { me, other: vec }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Guard<Interface>> {
        std::iter::once(&self.me).chain(self.other.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Guard<Interface>> {
        std::iter::once(&mut self.me).chain(self.other.iter_mut())
    }
}

impl IntoIterator for InterfacesGuard {
    type Item = Guard<Interface>;
    type IntoIter = std::iter::Chain<std::iter::Once<Self::Item>, std::vec::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.me).chain(self.other.into_iter())
    }
}
