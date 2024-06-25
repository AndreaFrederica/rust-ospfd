use std::marker::PhantomData;
use std::net::Ipv4Addr;

use super::bits::*;
use super::lsa::*;

use bytes::Buf;
use ospf_macros::raw_packet;
use pnet_macros_support::types::*;

pub mod types {
    pub const HELLO_PACKET: u8 = 1;
    pub const DB_DESCRIPTION: u8 = 2;
    pub const LS_REQUEST: u8 = 3;
    pub const LS_UPDATE: u8 = 4;
    pub const LS_ACKNOWLEDGE: u8 = 5;
}

pub const fn message_type_string(ty: u8) -> &'static str {
    match ty {
        types::HELLO_PACKET => "Hello Packet",
        types::DB_DESCRIPTION => "Database Description Packet",
        types::LS_REQUEST => "Link State Request Packet",
        types::LS_UPDATE => "Link State Update Packet",
        types::LS_ACKNOWLEDGE => "Link State Acknowledge Packet",
        _ => "Unknown",
    }
}

pub mod options {
    #[doc = "该位描述是否洪泛 AS-external-LSA，在本备忘录的第 3.6、9.5、10.8 和 12.1.2 节中描述。"]
    pub const E: u8 = 0b0000_0010;
    #[doc = "该位描述是否按照［引用 18］的说明转发 IP 多播包。"]
    pub const MC: u8 = 0b0000_0100;
    #[doc = "该位描述了处理类型 7 LSA，见［引用 19］的说明。"]
    pub const NP: u8 = 0b0000_1000;
    #[doc = "该位描述了是否按［引用 20］的说明忽略还是接收并转发 External-Attributes-LSA。"]
    pub const EA: u8 = 0b0001_0000;
    #[doc = "该位描述了按［引用 21］的说明处理按需链路。"]
    pub const DC: u8 = 0b0010_0000;

    pub trait OptionExt {
        fn is_set(&self, option: u8) -> bool;
        fn set(&mut self, option: u8);
        fn unset(&mut self, option: u8);
    }
}

macro_rules! option_ext {
    ($t:ty) => {
        impl options::OptionExt for $t {
            fn is_set(&self, option: u8) -> bool {
                (self.options & option) != 0
            }
            fn set(&mut self, option: u8) {
                self.options |= option;
            }
            fn unset(&mut self, option: u8) {
                self.options &= !option;
            }
        }
    };
}

/// Represents a OSPF Hello Packet.
#[raw_packet]
pub struct HelloPacket {
    pub network_mask: Ipv4Addr,
    pub hello_interval: u16,
    pub options: u8,
    pub router_priority: u8,
    pub router_dead_interval: u32,
    pub designated_router: Ipv4Addr,
    pub backup_designated_router: Ipv4Addr,
    pub neighbors: Vec<Ipv4Addr>,
}

/// Represents a OSPF Database Description Packet.
#[raw_packet]
pub struct DBDescription {
    pub interface_mtu: u16,
    pub options: u8,
    pub _zeros: PhantomData<u5>,
    pub init: u1,
    pub more: u1,
    pub master: u1,
    pub db_sequence_number: u32,
    pub lsa_header: Vec<LsaHeader>,
}

/// Represents a OSPF Link State Request Packet.
#[raw_packet]
pub struct LSRequest {
    pub ls_type: u32,
    pub ls_id: Ipv4Addr,
    pub advertising_router: Ipv4Addr,
}

/// Represents a OSPF Link State Update Packet.
#[raw_packet]
pub struct LSUpdate {
    pub num_lsa: u32,
    #[size(num_lsa)]
    pub lsa: Vec<Lsa>,
}

/// Represents a OSPF Link State Acknowledge Packet.
#[raw_packet]
pub struct LSAcknowledge {
    pub lsa_header: Vec<LsaHeader>,
}

option_ext!(HelloPacket);
option_ext!(DBDescription);

pub trait OspfSubPacket: ToBytes + ToBytesMut + FromBuf + std::fmt::Debug {
    fn get_type(&self) -> u8;

    fn get_type_string(&self) -> &'static str {
        message_type_string(self.get_type())
    }

    fn get_lsa_and_then(&self, f: impl FnMut(&Lsa)) {
        let _ = f;
    }
}

impl OspfSubPacket for HelloPacket {
    fn get_type(&self) -> u8 {
        types::HELLO_PACKET
    }
}

impl OspfSubPacket for DBDescription {
    fn get_type(&self) -> u8 {
        types::DB_DESCRIPTION
    }
}

impl OspfSubPacket for LSRequest {
    fn get_type(&self) -> u8 {
        types::LS_REQUEST
    }
}

impl OspfSubPacket for LSUpdate {
    fn get_type(&self) -> u8 {
        types::LS_UPDATE
    }

    fn get_lsa_and_then(&self, f: impl FnMut(&Lsa)) {
        self.lsa.iter().for_each(f);
    }
}

impl OspfSubPacket for LSAcknowledge {
    fn get_type(&self) -> u8 {
        types::LS_ACKNOWLEDGE
    }
}
