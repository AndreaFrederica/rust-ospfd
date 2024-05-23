use crate::bits::*;

use std::{marker::PhantomData, mem::size_of_val};

use bytes::{Buf, BytesMut};
use ospf_macros::raw_packet;
use pnet_macros_support::types::*;

pub mod types {
    pub const ROUTER_LSA: u8 = 1;
    pub const NETWORK_LSA: u8 = 2;
    pub const SUMMARY_IP_LSA: u8 = 3;
    pub const SUMMARY_ASBR_LSA: u8 = 4;
    pub const AS_EXTERNAL_LSA: u8 = 5;
}

#[derive(Clone, Debug)]
pub struct Lsa {
    pub header: LsaHeader,
    pub data: LsaData,
}

impl ToBytesMut for Lsa {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend(self.header.to_bytes());
        buf.extend(self.data.to_bytes());
        buf
    }
}

impl FromBuf for Lsa {
    fn from_buf(buf: &mut impl Buf) -> Self {
        let header = LsaHeader::from_buf(buf);
        let mut buf = buf.take(header.length as usize - size_of_val(&header));
        let data = match header.ls_type {
            types::ROUTER_LSA => LsaData::Router(RouterLSA::from_buf(&mut buf)),
            types::NETWORK_LSA => LsaData::Network(NetworkLSA::from_buf(&mut buf)),
            types::SUMMARY_IP_LSA => LsaData::SummaryIP(SummaryLSA::from_buf(&mut buf)),
            types::SUMMARY_ASBR_LSA => LsaData::SummaryASBR(SummaryLSA::from_buf(&mut buf)),
            types::AS_EXTERNAL_LSA => LsaData::ASExternal(AsExternalLSA::from_buf(&mut buf)),
            _ => panic!("wrong ls type!"),
        };
        assert!(!buf.has_remaining());
        Self { header, data }
    }
}

#[raw_packet]
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

#[derive(Clone, Debug)]
pub enum LsaData {
    Router(RouterLSA),
    Network(NetworkLSA),
    SummaryIP(SummaryLSA),
    SummaryASBR(SummaryLSA),
    ASExternal(AsExternalLSA),
}

impl ToBytesMut for LsaData {
    fn to_bytes_mut(&self) -> BytesMut {
        match self {
            LsaData::Router(lsa) => lsa.to_bytes_mut(),
            LsaData::Network(lsa) => lsa.to_bytes_mut(),
            LsaData::SummaryIP(lsa) => lsa.to_bytes_mut(),
            LsaData::SummaryASBR(lsa) => lsa.to_bytes_mut(),
            LsaData::ASExternal(lsa) => lsa.to_bytes_mut(),
        }
    }
}

#[raw_packet]
pub struct RouterLSA {
    pub flags: u16,
    pub num_links: u16,
    #[size(num_links)]
    pub links: Vec<RouterLSALink>,
}

#[raw_packet]
pub struct NetworkLSA {
    pub network_mask: u32,
    pub attached_routers: Vec<u32>,
}

#[raw_packet]
pub struct SummaryLSA {
    pub network_mask: u32,
    _zeros: PhantomData<u8>,
    pub metric: u24be,
    pub tos: u8,
    pub tos_metric: u24be,
}

#[raw_packet]
pub struct AsExternalLSA {
    pub network_mask: u32,
    pub e: u1,
    _zeros: PhantomData<u7>,
    pub metric: u24be,
    pub forwarding_address: u32,
    pub external_router_tag: u32,
}

#[raw_packet]
pub struct RouterLSALink {
    pub link_id: u32,
    pub link_data: u32,
    pub link_type: u8,
    pub tos: u8,
    pub metric: u16,
}
