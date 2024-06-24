use std::{collections::HashMap, net::Ipv4Addr};

use ospf_packet::lsa::LsaIndex;
use ospf_routing::{add_route as lib_add_route, delete_route, RoutingItem};

use crate::{area::Area, log, log_warning, util::ip2hex};

#[derive(Debug, Clone)]
pub struct RoutingTable {
    table: HashMap<Ipv4AddrMask, RoutingTableItem>,
}

impl RoutingTable {
    pub fn new() -> Self {
        RoutingTable {
            table: HashMap::new(),
        }
    }

    pub async fn recalculate(&mut self, areas: Vec<&mut Area>) {
        let old_table = std::mem::take(&mut self.table);
        for area in areas {
            area.recalc_routing().await;
            area.get_routing().into_iter().for_each(|item| {
                self.table
                    .insert(Ipv4AddrMask::from(item.dest_id, item.addr_mask), item);
            });
        }
        log_warning!("todo! calculate external routing");
        old_table.iter().for_each(|(k, old)| {
            let old = RoutingItem::from(old);
            let new = self.table.get(k);
            if !new.is_some_and(|new| old == new.into()) {
                log!("delete route: {:?}", delete_route(old));
            }
        });
        self.table.iter().for_each(|(k, new)| {
            let new = RoutingItem::from(new);
            if !old_table.get(k).is_some_and(|old| new == old.into()) {
                log!("add route: {:?}", add_route(new));
            }
        });
    }
}

/// insert a route into the routing table
/// if the route already exists, delete it first
fn add_route(r: RoutingItem) -> Result<(), std::io::Error> {
    use std::io::ErrorKind::AlreadyExists;
    match lib_add_route(r) {
        Err(e) if e.kind() == AlreadyExists => {
            delete_route(r)?;
            lib_add_route(r)
        }
        any => any,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ipv4AddrMask(Ipv4Addr, u8);

impl std::fmt::Debug for Ipv4AddrMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}/{:?}", self.0, self.1)
    }
}

impl std::fmt::Display for Ipv4AddrMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.0, self.1)
    }
}

impl Ipv4AddrMask {
    pub fn from(addr: Ipv4Addr, mask: Ipv4Addr) -> Self {
        let addr = addr & mask;
        Ipv4AddrMask(addr, ip2hex(mask).leading_ones() as u8)
    }

    pub fn mask(&self) -> Ipv4Addr {
        let mask = u32::MAX << (32 - self.1);
        Ipv4Addr::from(mask)
    }

    pub fn network(&self) -> Ipv4Addr {
        self.0 & self.mask()
    }
}

impl Default for Ipv4AddrMask {
    fn default() -> Self {
        Ipv4AddrMask(Ipv4Addr::UNSPECIFIED, 0)
    }
}

#[derive(Debug, Clone)]
pub struct RoutingTableItem {
    /// 目标类型/Destination Type
    pub dest_type: RoutingTableItemType,
    /// 目标标识/Destination ID
    pub dest_id: Ipv4Addr,
    /// 地址掩码/Address Mask
    pub addr_mask: Ipv4Addr,
    /// 可选项/Optional Capabilities
    pub external_cap: bool,
    /// 区域/Area
    pub area_id: Ipv4Addr,
    /// 路径类型/Path-type
    pub path_type: RoutingTablePathType,
    /// 距离值/Cost
    pub cost: u32,
    /// 类型 2 距离值/Type 2 cost
    pub cost_t2: u32,
    /// 连接状态起源/Link State Origin
    pub lsa_origin: LsaIndex,
    /// 下一跳/Next hop
    pub next_hop: Ipv4Addr,
    /// 宣告路由器/Advertising router
    pub ad_router: Ipv4Addr,
}

impl From<&RoutingTableItem> for RoutingItem {
    fn from(value: &RoutingTableItem) -> Self {
        Self {
            dest: value.dest_id,
            mask: value.addr_mask,
            nexthop: value.next_hop,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingTableItemType {
    Network,
    Router,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingTablePathType {
    /// 区域内路径
    AreaInternal,
    /// 区域间路径
    AreaExternal,
    /// 类型 1 外部路径
    AsExternalT1,
    /// 类型 2 外部路径
    AsExternalT2,
}

impl std::fmt::Display for RoutingTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "OSPF Routing Table")?;
        for item in self.table.values() {
            let Ok(item): Result<RoutingItem, _> = item.try_into() else {
                crate::log_error!("failed to convert RoutingTableItem to RoutingItem: {item:#?}");
                continue;
            };
            writeln!(f, "{}", item)?;
        }
        Ok(())
    }
}
