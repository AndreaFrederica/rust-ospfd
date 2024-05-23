use super::bits::*;
use super::lsa::*;

use ospf_macros::raw_packet;

pub mod types {
    pub const HELLO_PACKET: u8 = 1;
    pub const DB_DESCRIPTION: u8 = 2;
    pub const LS_REQUEST: u8 = 3;
    pub const LS_UPDATE: u8 = 4;
    pub const LS_ACKNOWLEDGE: u8 = 5;
}

/// Represents a OSPF Hello Packet.
#[raw_packet]
pub struct HelloPacket {
    pub network_mask: u32,
    pub hello_interval: u16,
    pub options: u8,
    pub router_priority: u8,
    pub router_dead_interval: u32,
    pub designated_router: u32,
    pub backup_designated_router: u32,
    pub neighbors: Vec<u32>,
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
    pub advertising_router: u32,
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
                    advertising_router: 0x04040404,
                    ls_sequence_number: 0x8000000b,
                    ls_checksum: 0xe6c8,
                    length: 48,
                },
                data: LsaData::Router(RouterLSA {
                    flags: 0,
                    num_links: 2,
                    links: vec![
                        RouterLSALink {
                            link_id: 0x04040404,
                            link_data: 0xffffffff,
                            link_type: 3,
                            tos: 0,
                            metric: 0,
                        },
                        RouterLSALink {
                            link_id: 0xa8010102,
                            link_data: 0xa8010102,
                            link_type: 2,
                            tos: 0,
                            metric: 1,
                        },
                    ],
                }),
            }],
        };
        assert_eq!(ls_update.to_bytes().to_vec(), raw_hex!("00000001000a020104040404040404048000000be6c800300000000204040404ffffffff03000000a8010102a801010202000001"));
    }
}
