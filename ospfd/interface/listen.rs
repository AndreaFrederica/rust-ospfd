use crate::{database::ProtocolDB, gen_lsa};

use super::{InterfaceEvent, InterfaceState, WInterface};

pub fn listen_interface(interface: WInterface) {
    tokio::spawn(async move {
        while let Some(interface) = interface.upgrade() {
            let mut interface = interface.lock().await;
            let net = interface.get_network_interface();
            if !net.is_up() {
                interface.interface_down().await;
            } else if net.is_loopback() {
                interface.loop_ind().await;
            } else if interface.state == InterfaceState::Loopback {
                interface.unloop_ind().await;
            } else {
                interface.interface_up().await;
            }
            //todo! temporary generate router lsa here
            let mut interfaces = ProtocolDB::upgrade_lock(interface).await;
            gen_lsa::gen_router_lsa(&mut interfaces).await;
            gen_lsa::gen_network_lsa(&mut interfaces).await;
            gen_lsa::gen_summary_lsa(&mut interfaces).await;
            drop(interfaces); // must release here, otherwise it will lock 8 secs...
            // check interface every 8 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;
        }
    });
}
