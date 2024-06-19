mod area;
mod capture;
mod constant;
mod daemon;
mod database;
mod flooding;
mod gen_lsa;
mod handler;
mod interface;
mod logging;
mod neighbor;
mod sender;
mod util;

use constant::BackboneArea;
use daemon::Daemon;
use database::ProtocolDB;
use interface::{AInterface, Interface};
use pnet::datalink::{self, NetworkInterface};

#[tokio::main()]
async fn main() {
    ProtocolDB::get().await.insert_area(BackboneArea).await;
    let interfaces: Vec<_> = datalink::interfaces().iter().filter_map(start).collect();
    if interfaces.is_empty() {
        panic!("No interface is available");
    }
    ProtocolDB::init(&interfaces);
    interfaces.iter().for_each(|i| Interface::start(i));

    loop {
        log_success!("{}", ProtocolDB::get().await.routing_table);
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
}

fn start(iface: &NetworkInterface) -> Option<AInterface> {
    if iface.ips.iter().find(|i| i.is_ipv4()).is_none() {
        log_warning!("The interface {} do NOT have an ipv4 address", iface.name);
        return None;
    };
    let interface = interface::Interface::from(&iface, BackboneArea);
    let ospf_handler = handler::ospf_handler_maker(interface.clone());
    let capture_daemon = capture::CaptureOspfDaemon::new(&iface, ospf_handler).unwrap();
    tokio::spawn(capture_daemon.run_forever());
    Some(interface)
}
