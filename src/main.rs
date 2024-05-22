mod macros;
mod packet;

use std::net::{IpAddr, Ipv4Addr};

use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::Packet;
use pnet::transport::transport_channel;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;

const BROADCAST_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(244, 0, 0, 5));

#[tokio::main()]
async fn main() {
    let protocol = Layer4(Ipv4(IpNextHeaderProtocols::OspfigP));

    // Create a new transport channel, dealing with layer 4 packets on a test protocol
    // It has a receive buffer of 4096 bytes.
    let (mut tx, _) = match transport_channel(4096, protocol) {
        Ok((tx, rx)) => (tx, rx),
        Err(e) => panic!(
            "An error occurred when creating the transport channel: {}",
            e
        ),
    };

    loop {
        let ospf_hello = packet::Ospf {
            version: 2,
            message_type: packet::packet::types::HELLO_PACKET,
            length: 44,
            router_id: 0x0a0a0a0a,
            area_id: 0,
            checksum: 0,
            au_type: 0,
            authentication: 0,
            payload: raw_hex!("ffffff00000a020100000028a801010100000000"),
        };
        let mut buffer = vec![0; 0x30];
        let mut packet = packet::MutableOspfPacket::new(&mut buffer).unwrap();
        packet.populate(&ospf_hello);
        let len = packet.packet().len();
        match tx.send_to(packet, BROADCAST_ADDR) {
            Ok(n) => assert_eq!(n, len),
            Err(e) => panic!("failed to send packet: {}", e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
