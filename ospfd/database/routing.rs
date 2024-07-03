use std::{collections::HashMap, net::Ipv4Addr};

use ospf_packet::lsa::LsaIndex;
use ospf_routing::{add_route as lib_add_route, delete_route as lib_delete_route, RoutingItem};

use crate::{
    area::Area,
    constant::{BackboneArea, LSInfinity},
    database::ProtocolDB,
    guard, log_error, must,
    util::ip2hex,
};

#[derive(Debug, Clone)]
pub struct RoutingTable {
    table: HashMap<RoutingTableIndex, RoutingTableItem>,
}

impl RoutingTable {
    pub fn new() -> Self {
        RoutingTable {
            table: HashMap::new(),
        }
    }

    pub async fn recalculate(&mut self, mut areas: Vec<&mut Area>) {
        let old_table = std::mem::take(&mut self.table);
        for area in areas.iter_mut() {
            area.recalc_routing();
            area.get_routing().into_iter().for_each(|item| {
                self.table
                    .entry(item.into())
                    .and_modify(|old| {
                        if item.better_than(old) {
                            *old = item;
                        }
                    })
                    .or_insert(item);
            });
        }
        // FIXME: 现在是全部重新计算，可以考虑使用增量计算
        // 另：目前不可能存在相同路径有多个的情况，会直接覆盖
        for area in areas.iter() {
            for item in area.get_routing_external().await {
                self.table
                    .entry(item.into())
                    .and_modify(|old| {
                        if item.better_than(old) {
                            *old = item;
                        }
                    })
                    .or_insert(item);
            }
        }
        // 传输区域计算暂未考虑
        // 计算 AS External
        for (header, lsa) in Area::get_all_external_lsa().await {
            use RoutingTableIndex::*;
            use RoutingTablePathType::*;
            must!(lsa.metric < LSInfinity; continue);
            must!(header.advertising_router != ProtocolDB::get_router_id(); continue);
            let addr = Ipv4AddrMask::from(header.link_state_id, lsa.network_mask);
            guard!(Some(asbr) = self.table.get(&AsbrRouter(header.advertising_router)); continue);
            let forwarding = if lsa.forwarding_address == Ipv4Addr::UNSPECIFIED {
                // forward to asbr
                asbr
            } else {
                // forward to forwarding_address
                guard!(Some(net) = self.get_routing(lsa.forwarding_address); continue);
                net
            };
            let t1 = lsa.e == 0; // is type 1 external routing
            let item = RoutingTableItem {
                dest_type: RoutingTableItemType::Network,
                dest_id: addr.network(),
                addr_mask: addr.mask(),
                external_cap: true,
                area_id: BackboneArea,
                path_type: if t1 { AsExternalT1 } else { AsExternalT2 },
                cost: forwarding.cost + if t1 { lsa.metric } else { 0 },
                cost_t2: forwarding.cost_t2 + if t1 { 0 } else { lsa.metric },
                lsa_origin: header.into(),
                next_hop: forwarding.next_hop,
                ad_router: header.advertising_router,
            };
            self.table
                .entry(item.into())
                .and_modify(|old| {
                    if item.better_than(old) {
                        *old = item;
                    }
                })
                .or_insert(item);
        }
        old_table.iter().for_each(|(k, old)| {
            guard!(Ok(old) = RoutingItem::try_from(old));
            let new = self.table.get(k);
            if !new.is_some_and(|new| new.try_into().is_ok_and(|new| old == new)) {
                delete_route(old).unwrap_or_else(|e| log_error!("Error(delete route): {:?}", e));
            }
        });
        self.table.iter().for_each(|(k, new)| {
            guard!(Ok(new) = RoutingItem::try_from(new));
            let old = old_table.get(k);
            if !old.is_some_and(|old| old.try_into().is_ok_and(|old| new == old)) {
                add_route(new).unwrap_or_else(|e| log_error!("Error(add route): {:?}", e));
            }
        });
    }

    pub fn get_routing(&self, ip: Ipv4Addr) -> Option<&RoutingTableItem> {
        (0..=32).rev().find_map(|mask| {
            let addr = Ipv4AddrMask(ip, mask);
            self.table.get(&RoutingTableIndex::Network(addr))
        })
    }

    pub fn get_routings(&self) -> Vec<&RoutingTableItem> {
        self.table.values().collect()
    }

    pub fn delete_all_routing(&self) {
        for item in self.table.values() {
            guard!(Ok(r) = RoutingItem::try_from(item); continue);
            delete_route(r).unwrap_or_else(|e| log_error!("Error(delete route): {:?}", e));
        }
    }
}

/// insert a route into the routing table
/// if the route already exists, delete it first
fn add_route(r: RoutingItem) -> Result<(), std::io::Error> {
    use std::io::ErrorKind::AlreadyExists;
    must!(r.nexthop != Ipv4Addr::UNSPECIFIED; ret: Ok(()));
    match lib_add_route(r) {
        Err(e) if e.kind() == AlreadyExists => {
            delete_route(r)?;
            lib_add_route(r)
        }
        any => any,
    }
}

fn delete_route(r: RoutingItem) -> Result<(), std::io::Error> {
    must!(r.nexthop != Ipv4Addr::UNSPECIFIED; ret: Ok(()));
    match lib_delete_route(r) {
        // route not exists
        Err(e) if e.raw_os_error() == Some(3) => Ok(()),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RoutingTableIndex {
    Network(Ipv4AddrMask),
    AsbrRouter(Ipv4Addr),
}

impl From<RoutingTableItem> for RoutingTableIndex {
    fn from(value: RoutingTableItem) -> Self {
        match value.dest_type {
            RoutingTableItemType::Network => {
                RoutingTableIndex::Network(Ipv4AddrMask::from(value.dest_id, value.addr_mask))
            }
            RoutingTableItemType::Router => RoutingTableIndex::AsbrRouter(value.dest_id),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

impl RoutingTableItem {
    pub fn better_than(&self, other: &Self) -> bool {
        match self.path_type.cmp(&other.path_type) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => match self.cost_t2.cmp(&other.cost_t2) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => false,
                std::cmp::Ordering::Greater => self.cost < other.cost,
            },
        }
    }
}

impl TryFrom<&RoutingTableItem> for RoutingItem {
    type Error = &'static str;
    fn try_from(value: &RoutingTableItem) -> Result<Self, Self::Error> {
        must!(value.dest_type == RoutingTableItemType::Network; ret: Err("not a network route"));
        Ok(Self {
            dest: value.dest_id,
            mask: value.addr_mask,
            nexthop: value.next_hop,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutingTableItemType {
    Network,
    Router,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
            guard!(Ok(r) = RoutingItem::try_from(item); continue);
            writeln!(
                f,
                "{}, cost: {}/{}, area: {}, type: {:?}",
                r, item.cost, item.cost_t2, item.area_id, item.path_type
            )?;
        }
        Ok(())
    }
}
