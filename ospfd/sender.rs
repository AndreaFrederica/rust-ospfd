use std::net::{IpAddr, Ipv4Addr};

use ospf_packet::{packet::OspfSubPacket, MutableOspfPacket, Ospf};
use pnet::packet::Packet as _;

use crate::{interface::AInterface, util::ip2hex};

async fn create_packet(interface: AInterface, packet: &impl OspfSubPacket) -> Ospf {
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

pub async fn send_packet(iface: AInterface, packet: &impl OspfSubPacket, destination: Ipv4Addr) {
    let raw = create_packet(iface.clone(), packet).await;
    let mut buffer = vec![0; raw.len()];
    let mut m_packet = MutableOspfPacket::new(&mut buffer).unwrap();
    m_packet.populate(&raw);
    m_packet.set_length(m_packet.packet().len() as u16);
    m_packet.auto_set_checksum();
    let pkg = m_packet.to_immutable();
    let mut ifw = iface.write().await;
    match ifw.sender.send_to(pkg, IpAddr::V4(destination)) {
        Ok(n) => assert_eq!(n, m_packet.packet().len()),
        Err(e) => panic!("failed to send packet: {}", e),
    }
    #[cfg(debug_assertions)]
    crate::log_success!("sent packet: {:#?}", packet);
}
