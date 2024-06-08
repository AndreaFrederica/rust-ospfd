use std::{net::Ipv4Addr, time::Duration};

use ospf_packet::packet::{DBDescription, LSRequest};

use crate::{guard, interface::Interface, must, sender::send_packet};

use super::Neighbor;

/// I think RefNeighbor is 100% safe, because it is impossible to borrow interface elsewhere
/// and it is impossible to borrow both neighbor and interface at one time.
pub struct RefNeighbor<'a> {
    interface: &'a mut Interface,
    neighbor: &'a mut Neighbor,
}

impl<'a> RefNeighbor<'a> {
    pub fn from(interface: &'a mut Interface, ip: Ipv4Addr) -> Option<Self> {
        unsafe {
            let interface = std::ptr::from_mut(interface);
            let neighbor = interface.as_mut().unwrap().neighbors.get_mut(&ip)?;
            Some(Self {
                interface: interface.as_mut().unwrap(),
                neighbor,
            })
        }
    }

    pub fn get_interface(&mut self) -> &mut Interface {
        self.interface
    }

    pub fn get_neighbor(&mut self) -> &mut Neighbor {
        self.neighbor
    }

    pub fn spawn_lsr_sender(&mut self) {
        must!(self.neighbor.lsr_handle.is_finished());
        let weak = self.interface.me.clone();
        let ip = self.neighbor.ip_addr;
        self.neighbor.lsr_handle = tokio::spawn(async move {
            while let Some(iface) = weak.upgrade() {
                let mut iface = iface.lock().await;
                guard!(Some(neighbor) = iface.neighbors.get_mut(&ip));
                guard!(Some(lsa) = neighbor.ls_request_list.pop_front());
                let packet = LSRequest {
                    ls_type: lsa.ls_type as u32,
                    ls_id: lsa.link_state_id,
                    advertising_router: lsa.advertising_router,
                };
                send_packet(&mut iface, &packet, ip).await;
                // yield to release the lock
                drop(iface);
                tokio::task::yield_now().await;
            }
        });
    }

    pub fn spawn_master_send_dd(&mut self, packet: DBDescription) {
        let weak = self.interface.me.clone();
        let ip = self.neighbor.ip_addr;
        use super::DdRxmt::Handle;
        self.neighbor.dd_rxmt.set(Handle(tokio::spawn(async move {
            while let Some(iface) = weak.upgrade() {
                let mut iface = iface.lock().await;
                must!(iface.neighbors.contains_key(&ip));
                send_packet(&mut iface, &packet, ip).await;
                // yield to release the lock
                let interval = Duration::from_secs(iface.rxmt_interval as u64);
                drop(iface);
                tokio::time::sleep(interval).await;
            }
        })));
    }
}
