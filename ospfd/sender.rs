use std::net::{IpAddr, Ipv4Addr};

use ospf_packet::{packet::OspfSubPacket, MutableOspfPacket, Ospf};
use pnet::packet::Packet as _;

use crate::{database::ProtocolDB, interface::Interface, util::ip2hex};

async fn create_packet(interface: &Interface, packet: &impl OspfSubPacket) -> Ospf {
    Ospf {
        version: 2,
        message_type: packet.get_type(),
        length: 0, // assign later
        router_id: ip2hex(ProtocolDB::get_router_id()),
        area_id: ip2hex(interface.area_id),
        checksum: 0, // assign later
        au_type: interface.au_type,
        authentication: interface.au_key,
        payload: packet.to_bytes().to_vec(),
    }
}

pub async fn send_packet(
    iface: &mut Interface,
    packet: &impl OspfSubPacket,
    destination: Ipv4Addr,
) {
    let raw = create_packet(iface, packet).await;
    let mut buffer = vec![0; raw.len()];
    let mut m_packet = MutableOspfPacket::new(&mut buffer).unwrap();
    m_packet.populate(&raw);
    m_packet.set_length(m_packet.packet().len() as u16);
    m_packet.auto_set_checksum();
    let pkg = m_packet.to_immutable();
    match iface.sender.send_to(pkg, IpAddr::V4(destination)) {
        Ok(n) => assert_eq!(n, m_packet.packet().len()),
        Err(e) => panic!("failed to send packet: {}", e),
    }
    // try update lsa in database
    tokio::task::block_in_place(|| packet.get_lsa_and_then(|lsa| {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(
            rt.block_on(ProtocolDB::get())
                .lsa_has_sent(iface.area_id, lsa),
        )
    }));
    #[cfg(debug_assertions)]
    crate::log!(
        "sent packet to {}: {}({} bytes)",
        destination,
        packet.get_type_string(),
        m_packet.packet().len()
    );
}
