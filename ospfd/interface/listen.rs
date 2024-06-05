use super::{InterfaceEvent, InterfaceState, WInterface};

pub fn listen_interface(interface: WInterface) {
    tokio::spawn(async move {
        while let Some(interface) = interface.upgrade() {
            let net = interface.read().await.get_network_interface();
            if !net.is_up() {
                interface.interface_down().await;
            } else if net.is_loopback() {
                interface.loop_ind().await;
            } else if interface.read().await.state == InterfaceState::Loopback {
                interface.unloop_ind().await;
            } else {
                interface.interface_up().await;
            }
            // check interface every 2 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
}
