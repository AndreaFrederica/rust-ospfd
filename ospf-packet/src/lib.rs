pub mod bits;
pub mod lsa;
pub mod packet;

pub use bits::{FromBuf, ToBytes, ToBytesMut};

use std::io;
use std::io::ErrorKind;
use std::mem;
use std::net::{self, IpAddr};
use std::time::Duration;

use pnet::packet::ipv4::Ipv4Packet;
use pnet::transport::transport_channel_iterator;
use pnet::transport::TransportChannelType::{Layer3, Layer4};
use pnet::transport::TransportProtocol::Ipv4;
use pnet::transport::TransportReceiver;
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

impl Ospf {
    pub fn len(&self) -> usize {
        24 + self.payload.len()
    }
}

transport_channel_iterator!(OspfPacket, OspfTransportChannelIterator, ospf_packet_iter);
