use std::{future::Future, net::Ipv4Addr, time::Duration};

use ospf_packet::packet::{DBDescription, LSRequest};

use crate::{
    guard, interface::Interface, must, neighbor::NeighborEvent, sender::send_packet,
    util::do_nothing_handle,
};

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

    /// Spawn a coroutine to send lsr.  
    /// This coroutine peek one lsa in ls_request_list one time and create a sub-coroutine
    /// to send this packet every rxmt_interval seconds, then the master coroutine will
    /// wait until the child finished.  
    /// If we receive a lsu for the front of ls_request_list, call lsr_recv_update to pop
    /// the corresponding lsa and stop the child coroutine, which will make the master
    /// coroutine peek another lsa.  
    /// When there are no more item in ls_request_list, the neighbor's event loading_done
    /// will be invoked.
    pub fn spawn_lsr_sender(&mut self) {
        must!(self.neighbor.lsr_handle.is_finished());
        let weak = self.interface.me.clone();
        let ip = self.neighbor.ip_addr;
        self.neighbor.lsr_handle.set(async move {
            while let Some(iface) = weak.upgrade() {
                let mut iface = iface.lock().await;
                guard!(Some(neighbor) = iface.neighbors.get_mut(&ip));
                guard! {
                    Some(lsa) = neighbor.ls_request_list.front();
                    else: RefNeighbor::from(&mut iface, ip).unwrap().loading_done().await;
                };
                let packet = LSRequest {
                    ls_type: lsa.ls_type as u32,
                    ls_id: lsa.link_state_id,
                    advertising_router: lsa.advertising_router,
                };
                // create child process to send packet every rxmt secs
                let weak = weak.clone();
                let rxmt = tokio::spawn(async move {
                    while let Some(iface) = weak.upgrade() {
                        let mut iface = iface.lock().await;
                        must!(iface.neighbors.contains_key(&ip));
                        send_packet(&mut iface, &packet, ip).await;
                        // drop to release the lock
                        let interval = Duration::from_secs(iface.rxmt_interval as u64);
                        drop(iface);
                        tokio::time::sleep(interval).await;
                    }
                });
                neighbor.lsr_handle.child = rxmt.abort_handle();
                // yield to release the lock
                drop(iface);
                let _ = rxmt.await;
            }
        });
    }

    pub fn lsr_recv_update(&mut self) {
        self.neighbor.ls_request_list.pop_front();
        self.neighbor.lsr_handle.child.abort();
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
                // drop to release the lock
                let interval = Duration::from_secs(iface.rxmt_interval as u64);
                drop(iface);
                tokio::time::sleep(interval).await;
            }
        })));
    }
}

#[derive(Debug)]
pub struct LsrHandle {
    master: tokio::task::AbortHandle,
    child: tokio::task::AbortHandle,
}

impl Default for LsrHandle {
    fn default() -> Self {
        Self {
            master: do_nothing_handle(),
            child: do_nothing_handle(),
        }
    }
}

impl LsrHandle {
    pub fn set<F>(&mut self, f: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.abort();
        self.master = tokio::spawn(f).abort_handle();
    }

    pub fn abort(&self) {
        self.master.abort();
        self.child.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.master.is_finished()
    }

    pub fn child_abort(&self) {
        self.child.abort();
    }
}
