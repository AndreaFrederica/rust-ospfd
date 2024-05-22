mod bits;
pub mod lsa;
pub mod packet;

use pnet_macros::packet;
use pnet_macros_support::types::*;

/// Represents a OSPF Packet.
#[packet]
pub struct Ospf {
    pub version: u8,
    pub message_type: u8,
    pub length: u16be,
    pub router_id: u32be,
    pub area_id: u32be,
    pub checksum: u16be,
    pub au_type: u16be,
    pub authentication: u64be,
    #[payload]
    pub payload: Vec<u8>, // the message type specific packet
}
