use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    net::Ipv4Addr,
};

use ospf_packet::lsa::{link_types::*, types::*, *};

use crate::{constant::LsaMaxAge, database::*, guard, must};

use super::Area;

#[derive(Debug)]
pub struct ShortPathTree {
    nodes: HashMap<NodeAddr, TreeNode>,
}

impl ShortPathTree {
    pub fn new() -> Self {
        ShortPathTree {
            nodes: HashMap::new(),
        }
    }

    pub async fn calculate(area: &mut Area) -> Self {
        let router_id = ProtocolDB::get_router_id();
        // 0. 数据库拷贝
        let lsa_briefs = area.get_all_lsa().await;
        let get_lsa = |header: LsaHeader| area.m_get_lsa(&HashMap::new(), header.into()).unwrap().0;
        let mut lsa_db: HashMap<_, _> = lsa_briefs
            .iter()
            .filter_map(|lsa| {
                if lsa.ls_age == LsaMaxAge {
                    return None;
                }
                match lsa.ls_type {
                    ROUTER_LSA => Some((NodeAddr::Router(lsa.link_state_id), get_lsa(*lsa))),
                    NETWORK_LSA => Some((NodeAddr::Network(lsa.link_state_id), get_lsa(*lsa))),
                    _ => None,
                }
            })
            .collect();
        let edges: HashMap<_, _> = lsa_db
            .iter()
            .map(|(&id, lsa)| (id, lsa2nodes(lsa)))
            .collect();
        edges.iter().for_each(|(id, map)| {
            map.keys().for_each(|&dest| {
                let lsa = lsa_db.get(id).unwrap().clone();
                lsa_db.entry(dest).or_insert(lsa);
            })
        });
        // 1. 初始化算法数据结构
        area.transit_capability = false;
        let mut tree = Self::new();
        let mut candidate = BinaryHeap::new();
        candidate.push(HeapNode(
            Reverse(0),
            false,
            NodeAddr::Router(router_id),
            vec![],
        ));
        // 2. 计算
        while let Some(next) = candidate.pop() {
            let HeapNode(Reverse(distance), _, id, nexthop) = next;
            tree.nodes.entry(id).and_modify(|node| {
                // 一样远的，加一个 nexthop。不可能更近
                must!(node.distance == distance);
                node.next_hops.extend(&nexthop);
            });
            must!(!tree.nodes.contains_key(&id); continue);
            let lsa = lsa_db.get(&id).unwrap();
            tree.nodes
                .insert(id, TreeNode::new(id, lsa.clone(), distance, nexthop));
            must!(!matches!(id, NodeAddr::Stub(_)); continue);
            let node = tree.nodes.get(&id).unwrap();
            let children = edges.get(&id).unwrap();
            area.transit_capability |= lsa_have_v(lsa);
            for (&child, &cost) in children {
                // 已在树中
                must!(!tree.nodes.contains_key(&child); continue);
                let distance = if matches!(child, NodeAddr::Stub(_)) {
                    // 存根网络不需要双向边
                    distance + cost
                } else {
                    // 存在双向边
                    guard!(Some(&c) = edges.get(&child).and_then(|m| m.get(&id)); continue);
                    distance + cost.max(c)
                };
                // 优先队列
                candidate.push(HeapNode(
                    Reverse(distance),
                    matches!(id, NodeAddr::Network(_)),
                    child,
                    tree.calc_nexthop(child, lsa_db.get(&child).unwrap(), node),
                ));
            }
        }
        tree
    }

    fn calc_nexthop(&self, node: NodeAddr, lsa: &Lsa, parent: &TreeNode) -> Vec<Ipv4Addr> {
        if parent.next_hops.is_empty() {
            match node {
                NodeAddr::Router(_) => {
                    guard!(NodeAddr::Network(network) = parent.id; ret: vec![]);
                    guard!(LsaData::Router(ref lsa) = lsa.data; ret: vec![]);
                    lsa.links.iter().find(|link| link.link_type == TRANSIT_LINK && link.link_id == network)
                        .map(|link| vec![link.link_data])
                        .unwrap_or(vec![])
                },
                _ => vec![],
            }
        } else {
            parent.next_hops.clone()
        }
    }

    pub fn get_routing(area: &Area) -> Vec<RoutingTableItem> {
        area.short_path_tree
            .nodes
            .values()
            .filter_map(|node| {
                let addr = match node.id {
                    NodeAddr::Router(_) => None,
                    NodeAddr::Network(ip) => {
                        let (_, network): (LsaHeader, NetworkLSA) =
                            node.lsa.clone().try_into().unwrap();
                        Some(Ipv4AddrMask::from(ip, network.network_mask))
                    }
                    NodeAddr::Stub(ip) => Some(ip),
                };
                must!(!node.next_hops.is_empty(); ret: None);
                guard!(Some(addr) = addr; ret: None);
                Some(RoutingTableItem {
                    dest_type: RoutingTableItemType::Network,
                    dest_id: addr.network(),
                    addr_mask: addr.mask(),
                    external_cap: area.external_routing_capability,
                    area_id: area.area_id,
                    path_type: RoutingTablePathType::AreaExternal,
                    cost: node.distance,
                    cost_t2: 0,
                    lsa_origin: node.lsa.header.into(),
                    next_hop: node.next_hops[0],
                    ad_router: node.lsa.header.advertising_router,
                })
            })
            .collect()
    }
}

fn lsa_have_v(lsa: &Lsa) -> bool {
    match lsa.data {
        LsaData::Router(ref lsa) => lsa.v != 0,
        _ => false,
    }
}

/// 获得 LSA 指明的所有邻接节点
fn lsa2nodes(lsa: &Lsa) -> HashMap<NodeAddr, u32> {
    let mut map = HashMap::new();
    match lsa.data {
        LsaData::Router(ref lsa) => {
            for link in &lsa.links {
                match link.link_type {
                    STUB_LINK => {
                        map.insert(
                            NodeAddr::Stub(Ipv4AddrMask::from(link.link_id, link.link_data)),
                            link.metric as u32,
                        );
                    }
                    P2P_LINK | VIRTUAL_LINK => {
                        map.insert(NodeAddr::Router(link.link_id), link.metric as u32);
                    }
                    TRANSIT_LINK => {
                        map.insert(NodeAddr::Network(link.link_id), link.metric as u32);
                    }
                    _ => unreachable!(),
                }
            }
        }
        LsaData::Network(ref lsa) => {
            for &id in lsa.attached_routers.iter() {
                map.insert(NodeAddr::Router(id), 0);
            }
        }
        _ => unreachable!(),
    }
    map
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// (distance, is_network, node_id, next_hop)
struct HeapNode(Reverse<u32>, bool, NodeAddr, Vec<Ipv4Addr>);

#[derive(Debug)]
struct TreeNode {
    id: NodeAddr,
    lsa: Lsa,
    /// (interface index, next hop ip)
    next_hops: Vec<Ipv4Addr>,
    distance: u32,
}

impl TreeNode {
    fn new(id: NodeAddr, lsa: Lsa, distance: u32, next_hops: Vec<Ipv4Addr>) -> Self {
        Self {
            id,
            lsa,
            next_hops,
            distance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum NodeAddr {
    /// A router with router id
    Router(Ipv4Addr),
    /// Transit network with DR ip
    Network(Ipv4Addr),
    /// Stub network with ip and mask
    Stub(Ipv4AddrMask),
}
