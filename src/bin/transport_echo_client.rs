// Copyright (c) 2014, 2015 Robert Clipsham <robert@octarineparrot.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A simple client for packets using a test protocol
extern crate pnet;

use std::net::Ipv4Addr;

use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::udp::MutableUdpPacket;
use pnet::packet::Packet;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;
use pnet::transport::{transport_channel, udp_packet_iter};

fn main() {
    let protocol = Layer4(Ipv4(IpNextHeaderProtocols::Test1));

    // Create a new transport channel, dealing with layer 4 packets on a test protocol
    // It has a receive buffer of 4096 bytes.
    let (mut tx, mut rx) = match transport_channel(4096, protocol) {
        Ok((tx, rx)) => (tx, rx),
        Err(e) => panic!(
            "An error occurred when creating the transport channel: {}",
            e
        ),
    };

    // We treat received packets as if they were UDP packets
    let mut iter = udp_packet_iter(&mut rx);
    let dest = std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    for idx in 0..5 {
        let mut msg = vec![0xdb, 0x03, 0x00, 0x50, 0x00, 0x0c, 0xa2, 0x60, 0x68, 0x69, 0x30 + idx, 0x0A];
        let packet = MutableUdpPacket::new(&mut msg).unwrap();
        match tx.send_to(packet, dest) {
            Ok(n) => println!("send => {} bytes", n),
            Err(e) => panic!("failed to send packet: {}", e),
        }
    }
    for _ in 0..10 {
        match iter.next() {
            Ok((packet, addr)) => {
                // println
                println!("{:?} => {:?}, payload: {}", addr, packet, String::from_utf8_lossy(packet.payload()));
            }
            Err(e) => {
                // If an error occurs, we can handle it here
                panic!("An error occurred while reading: {}", e);
            }
        }
    }
}