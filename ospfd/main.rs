mod capture;
mod constant;
mod daemon;
mod handler;
mod interface;
mod logging;
mod neighbor;
mod types;
mod util;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use constant::{AllSPFRouters, BackboneArea};
use daemon::Daemon;
use interface::Interface;
use ospf_packet::*;
use pnet::datalink;
use pnet::packet::ip::IpNextHeaderProtocols::OspfigP;
use pnet::packet::Packet;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;
use pnet::transport::{transport_channel, TransportSender};
use tokio::sync::RwLock;
use util::ip2hex;

#[tokio::main()]
async fn main() {
    let iface = datalink::interfaces()
        .into_iter()
        .filter(|i| i.name == "eth0")
        .next()
        .expect("There is no interface named eth0");
    let interface = interface::Interface::from(&iface);
    let ospf_handler = handler::ospf_handler_maker(interface.clone());
    let capture_daemon = capture::CaptureOspfDaemon::new(&iface, ospf_handler).unwrap();

    let (tx, _) = match transport_channel(4096, Layer4(Ipv4(OspfigP))) {
        Ok((tx, rx)) => (tx, rx),
        Err(e) => panic!(
            "An error occurred when creating the transport channel: {}",
            e
        ),
    };

    let h1 = tokio::spawn(hello(interface, tx));
    let h2 = tokio::spawn(capture_daemon.run_forever());
    h1.await.unwrap();
    h2.await.unwrap();
}

async fn hello(interface: Arc<RwLock<Interface>>, mut tx: TransportSender) {
    loop {
        let router_id = interface.read().await.router_id;
        let neighbors = interface.read().await.neighbors.clone();
        let ospf_hello = Ospf {
            version: 2,
            message_type: packet::types::HELLO_PACKET,
            length: 0,
            router_id: ip2hex(router_id),
            area_id: BackboneArea,
            checksum: 0,
            au_type: 0,
            authentication: 0,
            payload: packet::HelloPacket {
                network_mask: Ipv4Addr::new(255, 255, 255, 0),
                hello_interval: 10,
                options: packet::options::E,
                router_priority: 1,
                router_dead_interval: 40,
                designated_router: interface.read().await.ip_addr,
                backup_designated_router: Ipv4Addr::new(0, 0, 0, 0),
                neighbors,
            }
            .to_bytes()
            .to_vec(),
        };
        let mut buffer = vec![0; ospf_hello.len()];
        let mut packet = MutableOspfPacket::new(&mut buffer).unwrap();
        packet.populate(&ospf_hello);
        packet.set_length(packet.packet().len() as u16);
        packet.auto_set_checksum();
        let len = packet.packet().len();
        match tx.send_to(packet, IpAddr::V4(AllSPFRouters)) {
            Ok(n) => assert_eq!(n, len),
            Err(e) => panic!("failed to send packet: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
