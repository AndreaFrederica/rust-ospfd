mod capture;
mod constant;
mod logging;
mod util;

use std::sync::Arc;
use std::time::Duration;

use constant::AllSPFRouters;
use ospf_packet::*;
use pnet::packet::ip::IpNextHeaderProtocols::OspfigP;
use pnet::packet::Packet;
use pnet::transport::TransportChannelType::Layer4;
use pnet::transport::TransportProtocol::Ipv4;
use pnet::transport::{transport_channel, TransportSender};
use tokio::sync::Mutex;

#[tokio::main()]
async fn main() {
    let capture_daemon = capture::CaptureOspfDaemon::new("eth0", capture::echo_handler).unwrap();

    let (tx, _) = match transport_channel(4096, Layer4(Ipv4(OspfigP))) {
        Ok((tx, rx)) => (tx, rx),
        Err(e) => panic!(
            "An error occurred when creating the transport channel: {}",
            e
        ),
    };

    let tx = Arc::new(Mutex::new(tx));
    let h1 = tokio::spawn(hello(tx.clone()));
    let h2 = tokio::spawn(capture_daemon.capture_forever());
    h1.await.unwrap();
    h2.await.unwrap();
}

async fn hello(tx: Arc<Mutex<TransportSender>>) {
    loop {
        let ospf_hello = Ospf {
            version: 2,
            message_type: packet::types::HELLO_PACKET,
            length: 44,
            router_id: hex!(10, 10, 10, 10),
            area_id: hex!(0, 0, 0, 0),
            checksum: 0,
            au_type: 0,
            authentication: 0,
            payload: packet::HelloPacket {
                network_mask: hex!(255, 255, 255, 0),
                hello_interval: 10,
                options: packet::options::E,
                router_priority: 1,
                router_dead_interval: 40,
                designated_router: hex!(10, 10, 10, 10),
                backup_designated_router: hex!(0, 0, 0, 0),
                neighbors: vec![],
            }
            .to_bytes()
            .to_vec(),
        };
        let mut buffer = vec![0; ospf_hello.len()];
        let mut packet = MutableOspfPacket::new(&mut buffer).unwrap();
        packet.populate(&ospf_hello);
        packet.auto_set_checksum();
        let len = packet.packet().len();
        match tx.lock().await.send_to(packet, ip!(AllSPFRouters)) {
            Ok(n) => assert_eq!(n, len),
            Err(e) => panic!("failed to send packet: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
