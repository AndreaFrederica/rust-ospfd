use crate::{bits::*, constant};

use std::{marker::PhantomData, mem::size_of_val, net::Ipv4Addr};

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

impl Lsa {
    pub fn checksum(&self) -> u16 {
        let mut c0 = 0u16;
        let mut c1 = 0u16;
        let mut buf = self.to_bytes_mut();
        buf[16] = 0; // ls_checksum
        buf[17] = 0; // ls_checksum
        let _ = buf.split_to(2); // drop ls_age
        for &byte in buf.as_ref() {
            c0 = (c0 + byte as u16) % 0xFF;
            c1 = (c1 + c0) % 0xFF;
        }
        let (c0, c1) = (c0 as i32, c1 as i32);
        let mul = (self.header.length as i32 - 16) * c0;
        let mut x = mul - c0 - c1;
        let mut y = c1 - mul - 1;
        if y >= 0 { y += 1; }
        if x < 0 { x -= 1; }
        x %= 255;
        y %= 255;
        if x == 0 { x = 255; }
        if y == 0 { y = 255; }
        y &= 0x00FF;
        let (x, y) = (x as u16, y as u16);
        (x << 8) | y
    }

    pub fn checksum_ok(&self) -> bool {
        self.checksum() == self.header.ls_checksum
    }

    pub fn update_checksum(&mut self) {
        self.header.ls_checksum = self.checksum();
    }

    pub fn update_length(&mut self) {
        self.header.length = self.to_bytes().len() as u16;
    }
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
#[derive(Eq, Copy)]
#[doc = "The LsaHeader impl Ord, the greater the newer"]
pub struct LsaHeader {
    pub ls_age: u16,
    pub options: u8,
    pub ls_type: u8,
    pub link_state_id: Ipv4Addr,
    pub advertising_router: Ipv4Addr,
    pub ls_sequence_number: i32,
    pub ls_checksum: u16,
    pub length: u16,
}

impl Ord for LsaHeader {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // 较大 LS 序号的 LSA 较新
        if self.ls_sequence_number != other.ls_sequence_number {
            return self.ls_sequence_number.cmp(&other.ls_sequence_number);
        }
        // 具有较大校验和（按 16 位无符号整数）的实例较新
        if self.ls_checksum != other.ls_checksum {
            return self.ls_checksum.cmp(&other.ls_checksum);
        }
        // 如果其中一个实例的 LS 时限为 MaxAge，则这个实例为较新
        if self.ls_age == other.ls_age {
            return std::cmp::Ordering::Equal;
        }
        if self.ls_age == constant::LsaMaxAge {
            return std::cmp::Ordering::Greater;
        }
        if other.ls_age == constant::LsaMaxAge {
            return std::cmp::Ordering::Greater;
        }
        // 如果两个实例 LS 时限的差异大于 MaxAgeDiff，较小时限（较近生成）的实例为较新
        if self.ls_age.abs_diff(other.ls_age) >= constant::MaxAgeDiff {
            return other.ls_age.cmp(&self.ls_age);
        }
        std::cmp::Ordering::Equal
    }
}

impl PartialOrd for LsaHeader {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for LsaHeader {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

#[derive(Clone, Debug)]
#[derive(PartialEq, Eq)]
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
#[derive(PartialEq, Eq)]
pub struct RouterLSA {
    pub _z1: PhantomData<u5>,
    pub v: u1,
    pub e: u1,
    pub b: u1,
    pub _z2: PhantomData<u8>,
    pub num_links: u16,
    #[size(num_links)]
    pub links: Vec<RouterLSALink>,
}

#[raw_packet]
#[derive(PartialEq, Eq)]
pub struct NetworkLSA {
    pub network_mask: Ipv4Addr,
    pub attached_routers: Vec<Ipv4Addr>,
}

#[raw_packet]
#[derive(PartialEq, Eq)]
pub struct SummaryLSA {
    pub network_mask: Ipv4Addr,
    pub _zeros: PhantomData<u8>,
    pub metric: u24be,
    pub tos: u8,
    pub tos_metric: u24be,
}

#[raw_packet]
#[derive(PartialEq, Eq)]
pub struct AsExternalLSA {
    pub network_mask: Ipv4Addr,
    pub e: u1,
    pub _zeros: PhantomData<u7>,
    pub metric: u24be,
    pub forwarding_address: Ipv4Addr,
    pub external_router_tag: u32,
}

pub mod link_types {
    pub const P2P_LINK: u8 = 1;
    pub const TRANSIT_LINK: u8 = 2;
    pub const STUB_LINK: u8 = 3;
    pub const VIRTUAL_LINK: u8 = 4;
}

#[raw_packet]
#[derive(PartialEq, Eq)]
pub struct RouterLSALink {
    pub link_id: Ipv4Addr,
    pub link_data: Ipv4Addr,
    pub link_type: u8,
    pub tos: u8,
    pub metric: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("Unknown lsa type")]
    TypeUnknown,
    #[error("Lsa type mismatched")]
    TypeMismatched,
}

macro_rules! unpack {
    ($x:expr, $i:ident) => {
        if let $i(y) = $x {
            y
        } else {
            return Err(ConvertError::TypeMismatched);
        }
    };
}

macro_rules! build_convert {
    ($T:ty, $(($id:ident, $e:ident)),+) => {
impl TryFrom<(LsaHeader, $T)> for Lsa {
    type Error = ConvertError;

    fn try_from((header, data): (LsaHeader, $T)) -> Result<Self, Self::Error> {
        use LsaData::*;
        match header.ls_type {
            $(types::$id => Ok(Self { header, data: $e(data) }),)+
            _x @ 1..=5 => Err(ConvertError::TypeMismatched),
            _ => Err(ConvertError::TypeUnknown),
        }
    }
}

impl TryFrom<Lsa> for (LsaHeader, $T) {
    type Error = ConvertError;

    fn try_from(lsa: Lsa) -> Result<Self, Self::Error> {
        use LsaData::*;
        match lsa.header.ls_type {
            $(types::$id => Ok((lsa.header, unpack!(lsa.data, $e))),)+
            _x @ 1..=5 => Err(ConvertError::TypeMismatched),
            _ => Err(ConvertError::TypeUnknown),
        }
    }
}
    };
}

build_convert!(RouterLSA, (ROUTER_LSA, Router));
build_convert!(NetworkLSA, (NETWORK_LSA, Network));
build_convert!(SummaryLSA, (SUMMARY_IP_LSA, SummaryIP), (SUMMARY_ASBR_LSA, SummaryASBR));
build_convert!(AsExternalLSA, (AS_EXTERNAL_LSA, ASExternal));

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LsaIndex {
    pub ls_type: u8,
    pub ls_id: Ipv4Addr,
    pub ad_router: Ipv4Addr,
}

impl LsaIndex {
    pub fn new(ls_type: u8, ls_id: Ipv4Addr, ad_router: Ipv4Addr) -> Self {
        Self {
            ls_type,
            ls_id,
            ad_router,
        }
    }
}

impl From<LsaHeader> for LsaIndex {
    fn from(value: LsaHeader) -> Self {
        Self {
            ls_type: value.ls_type,
            ls_id: value.link_state_id,
            ad_router: value.advertising_router,
        }
    }
}
