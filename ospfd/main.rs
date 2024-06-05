mod capture;
mod constant;
mod daemon;
mod handler;
mod interface;
mod logging;
mod neighbor;
mod sender;
mod types;
mod util;

use daemon::Daemon;
use interface::InterfaceEvent;
use pnet::datalink;

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

    interface.interface_up().await;
    let hd = tokio::spawn(capture_daemon.run_forever());
    hd.await.unwrap();
}
