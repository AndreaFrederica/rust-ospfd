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
#[derive(Eq)]
#[doc = "The LsaHeader impl Ord, the greater the newer"]
pub struct LsaHeader {
    pub ls_age: u16,
    pub options: u8,
    pub ls_type: u8,
    pub link_state_id: u32,
    pub advertising_router: Ipv4Addr,
    pub ls_sequence_number: u32,
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
    pub network_mask: Ipv4Addr,
    pub attached_routers: Vec<Ipv4Addr>,
}

#[raw_packet]
pub struct SummaryLSA {
    pub network_mask: Ipv4Addr,
    _zeros: PhantomData<u8>,
    pub metric: u24be,
    pub tos: u8,
    pub tos_metric: u24be,
}

#[raw_packet]
pub struct AsExternalLSA {
    pub network_mask: Ipv4Addr,
    pub e: u1,
    _zeros: PhantomData<u7>,
    pub metric: u24be,
    pub forwarding_address: Ipv4Addr,
    pub external_router_tag: u32,
}

#[raw_packet]
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
