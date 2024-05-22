use super::bits::{Bits, BitsMut};

use std::marker::PhantomData;

use pnet_macros_support::types::*;

#[derive(Clone, Debug)]
pub struct Lsa {
    pub header: LsaHeader,
    pub data: LsaData,
}

#[derive(Clone, Debug)]
pub struct LsaHeader {
    pub ls_age: u16,
    pub options: u8,
    pub ls_type: u8,
    pub link_state_id: u32,
    pub advertising_router: u32,
    pub ls_sequence_number: u32,
    pub ls_checksum: u16,
    pub length: u16,
}

pub mod types {
    pub const ROUTER_LSA: u8 = 1;
    pub const NETWORK_LSA: u8 = 2;
    pub const SUMMARY_IP_LSA: u8 = 3;
    pub const SUMMARY_ASBR_LSA: u8 = 4;
    pub const AS_EXTERNAL_LSA: u8 = 5;
}

#[derive(Clone, Debug)]
pub enum LsaData {
    Router(RouterLSA),
    Network(NetworkLSA),
    SummaryIP(SummaryLSA),
    SummaryASBR(SummaryLSA),
    ASExternal(AsExternalLSA),
}

#[derive(Clone, Debug)]
pub struct RouterLSA {
    pub flags: u16,
    pub num_links: u16,
    pub links: Vec<RouterLSALink>,
}

#[derive(Clone, Debug)]
pub struct NetworkLSA {
    pub network_mask: u32,
    pub attached_routers: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct SummaryLSA {
    pub network_mask: u32,
    _zeros: PhantomData<u8>,
    pub metric: u24be,
    pub tos: u8,
    pub tos_metric: u24be,
}

#[derive(Clone, Debug)]
pub struct AsExternalLSA {
    pub network_mask: u32,
    pub e: u1,
    _zeros: PhantomData<u7>,
    pub metric: u24be,
    pub forwarding_address: u32,
    pub external_router_tag: u32,
}

#[derive(Clone, Debug)]
pub struct RouterLSALink {
    pub link_id: u32,
    pub link_data: u32,
    pub link_type: u8,
    pub tos: u8,
    pub metric: u16,
}
