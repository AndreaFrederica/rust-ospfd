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
use pnet::packet::Packet;
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

impl MutableOspfPacket<'_> {
    pub fn auto_set_checksum(&mut self) {
        self.set_checksum(0);
        let checksum = self.packet().chunks(2).fold(0u16, |acc, e| {
            let e = u16::from_be_bytes(e.try_into().unwrap());
            let n = acc as u32 + e as u32;
            n as u16 + (n >> 16) as u16
        });
        self.set_checksum(checksum ^ 0xffff);
    }
}

impl OspfPacket<'_> {
    pub fn auto_test_checksum(&self) -> bool {
        let checksum = self.packet().chunks(2).fold(0u16, |acc, e| {
            let e = u16::from_be_bytes(e.try_into().unwrap());
            let n = acc as u32 + e as u32;
            n as u16 + (n >> 16) as u16
        });
        checksum == 0xffff
    }
}

transport_channel_iterator!(OspfPacket, OspfTransportChannelIterator, ospf_packet_iter);
