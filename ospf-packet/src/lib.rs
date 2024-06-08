pub mod bits;
mod constant;
pub mod lsa;
pub mod packet;

pub use bits::{FromBuf, ToBytes, ToBytesMut};
pub use packet::message_type_string;

use std::io;
use std::io::ErrorKind;
use std::mem;
use std::net::{self, IpAddr, Ipv4Addr};
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

impl From<OspfPacket<'_>> for Ospf {
    fn from(value: OspfPacket<'_>) -> Self {
        Self {
            version: value.get_version(),
            message_type: value.get_message_type(),
            length: value.get_length(),
            router_id: value.get_router_id(),
            area_id: value.get_area_id(),
            checksum: value.get_checksum(),
            au_type: value.get_au_type(),
            authentication: value.get_authentication(),
            payload: value.payload().to_vec(),
        }
    }
}

impl std::fmt::Display for MutableOspfPacket<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_immutable().fmt(f)
    }
}

macro_rules! ospf_fmt {
    () => {
"OspfPacket: {{
    version: {},
    message_type: {},
    length: {},
    router_id: {},
    area_id: {},
    checksum: {},
    au_type: {},
    authentication: {},
}}"
    };
}

pub const fn hex2ip(hex: u32) -> Ipv4Addr {
    let bytes = hex.to_be_bytes();
    Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
}

impl std::fmt::Display for OspfPacket<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f, ospf_fmt!(),
            self.get_version(),
            message_type_string(self.get_message_type()),
            self.get_length(),
            hex2ip(self.get_router_id()),
            self.get_area_id(),
            self.get_checksum(),
            self.get_au_type(),
            self.get_authentication()
        )
    }
}

transport_channel_iterator!(OspfPacket, OspfTransportChannelIterator, ospf_packet_iter);
