mod area;
mod capture;
mod constant;
mod daemon;
mod database;
mod handler;
mod interface;
mod logging;
mod neighbor;
mod sender;
mod types;
mod util;

use std::net::IpAddr;

use constant::BackboneArea;
use daemon::Daemon;
use database::ProtocolDB;
use pnet::datalink;

#[tokio::main()]
async fn main() {
    let iface = datalink::interfaces()
        .into_iter()
        .filter(|i| i.name == "eth0")
        .next()
        .expect("There is no interface named eth0");
    let IpAddr::V4(ip) = iface
        .ips
        .iter()
        .filter(|i| i.is_ipv4())
        .next()
        .unwrap()
        .ip()
    else {
        unreachable!();
    };
    ProtocolDB::init(ip);
    let interface = interface::Interface::from(&iface, BackboneArea).await;
    let ospf_handler = handler::ospf_handler_maker(interface.clone());
    let capture_daemon = capture::CaptureOspfDaemon::new(&iface, ospf_handler).unwrap();

    let hd = tokio::spawn(capture_daemon.run_forever());
    hd.await.unwrap();
}
