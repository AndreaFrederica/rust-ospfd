use std::net::Ipv4Addr;

use super::bits::*;
use super::lsa::*;

use bytes::Buf;
use ospf_macros::raw_packet;

pub mod types {
    pub const HELLO_PACKET: u8 = 1;
    pub const DB_DESCRIPTION: u8 = 2;
    pub const LS_REQUEST: u8 = 3;
    pub const LS_UPDATE: u8 = 4;
    pub const LS_ACKNOWLEDGE: u8 = 5;
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
    pub db_description: u8,
    pub db_sequence_number: u32,
    pub lsa_header: Vec<LsaHeader>,
}

/// Represents a OSPF Link State Request Packet.
#[raw_packet]
pub struct LSRequest {
    pub ls_type: u32,
    pub ls_id: u32,
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

pub trait OspfSubPacket: ToBytes + ToBytesMut + FromBuf {
    fn get_type(&self) -> u8;
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
}

impl OspfSubPacket for LSAcknowledge {
    fn get_type(&self) -> u8 {
        types::LS_ACKNOWLEDGE
    }
}

#[cfg(test)]
mod test {
    #![allow(non_snake_case)]
    use crate::lsa;

    use super::*;

    macro_rules! raw_hex {
        ($raw:literal) => {{
            let mut vec = Vec::new();
            let mut iter = $raw.chars();
            while let Some(c) = iter.next() {
                let byte = match iter.next() {
                    Some(c2) => u8::from_str_radix(&format!("{}{}", c, c2), 16).unwrap(),
                    None => panic!("Invalid raw hex string"),
                };
                vec.push(byte);
            }
            vec
        }};
    }

    #[test]
    fn test_LSUpdate() {
        let ls_update = LSUpdate {
            num_lsa: 1,
            lsa: vec![Lsa {
                header: LsaHeader {
                    ls_age: 10,
                    options: 0x0002,
                    ls_type: lsa::types::ROUTER_LSA,
                    link_state_id: 0x04040404,
                    advertising_router: Ipv4Addr::new(4, 5, 6, 7),
                    ls_sequence_number: 0x8000000b,
                    ls_checksum: 0xe6c8,
                    length: 48,
                },
                data: LsaData::Router(RouterLSA {
                    flags: 0,
                    num_links: 2,
                    links: vec![
                        RouterLSALink {
                            link_id: Ipv4Addr::new(4, 5, 6, 7),
                            link_data: Ipv4Addr::new(255, 255, 255, 255),
                            link_type: 3,
                            tos: 0,
                            metric: 0,
                        },
                        RouterLSALink {
                            link_id: Ipv4Addr::new(168, 1, 1, 2),
                            link_data: Ipv4Addr::new(168, 1, 1, 2),
                            link_type: 2,
                            tos: 0,
                            metric: 1,
                        },
                    ],
                }),
            }],
        };
        assert_eq!(ls_update.to_bytes().to_vec(), raw_hex!("00000001000a020104040404040506078000000be6c800300000000204050607ffffffff03000000a8010102a801010202000001"));
    }
}
