use std::net::{IpAddr, Ipv4Addr};

use ospf_packet::{packet::OspfSubPacket, MutableOspfPacket, Ospf};
use pnet::packet::Packet as _;

use crate::{interface::AInterface, util::ip2hex};

async fn create_packet(interface: AInterface, packet: impl OspfSubPacket) -> Ospf {
    let ifr = interface.read().await;
    Ospf {
        version: 2,
        message_type: packet.get_type(),
        length: 0, // assign later
        router_id: ip2hex(ifr.router_id),
        area_id: ip2hex(ifr.area_id),
        checksum: 0, // assign later
        au_type: ifr.au_type,
        authentication: ifr.au_key,
        payload: packet.to_bytes().to_vec(),
    }
}

pub async fn send_packet(interface: AInterface, packet: impl OspfSubPacket, destination: Ipv4Addr) {
    let raw = create_packet(interface.clone(), packet).await;
    let mut buffer = vec![0; raw.len()];
    let mut packet = MutableOspfPacket::new(&mut buffer).unwrap();
    packet.populate(&raw);
    packet.set_length(packet.packet().len() as u16);
    packet.auto_set_checksum();
    let len = packet.packet().len();
    let mut ifw = interface.write().await;
    match ifw.sender.send_to(packet, IpAddr::V4(destination)) {
        Ok(n) => assert_eq!(n, len),
        Err(e) => panic!("failed to send packet: {}", e),
    }
}
