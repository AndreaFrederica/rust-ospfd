#![allow(unused)]

use super::lsa::*;

pub mod types {
    pub const HELLO_PACKET: u8 = 1;
    pub const DB_DESCRIPTION: u8 = 2;
    pub const LS_REQUEST: u8 = 3;
    pub const LS_UPDATE: u8 = 4;
    pub const LS_ACKNOWLEDGE: u8 = 5;
}

/// Represents a OSPF Hello Packet.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub struct DBDescription {
    pub interface_mtu: u16,
    pub options: u8,
    pub db_description: u8,
    pub db_sequence_numr: u32,
    pub lsa_header: Vec<LSAHeader>,
}

/// Represents a OSPF Link State Request Packet.
#[derive(Clone, Debug)]
pub struct LSRequest {
    pub ls_type: u32,
    pub ls_id: u32,
    pub advertising_router: u32,
}

/// Represents a OSPF Link State Update Packet.
#[derive(Clone, Debug)]
pub struct LSUpdate {
    pub lsa_count: u32,
    pub lass: Vec<LSA>,
}

/// Represents a OSPF Link State Acknowledge Packet.
#[derive(Clone, Debug)]
pub struct LSAcknowledge {
    pub lsa_header: Vec<LSAHeader>,
}
