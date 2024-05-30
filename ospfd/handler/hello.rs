use std::sync::Arc;

use ospf_packet::packet::AddressedHelloPacket;
use tokio::sync::{mpsc, RwLock};

use crate::{daemon::AsyncRunnable, interface::Interface};

pub struct HelloHandler {
    iface: Arc<RwLock<Interface>>,
    rx: mpsc::Receiver<AddressedHelloPacket>,
}

impl HelloHandler {
    pub fn new(iface: Arc<RwLock<Interface>>, rx: mpsc::Receiver<AddressedHelloPacket>) -> Self {
        Self { iface, rx }
    }
}

impl AsyncRunnable for HelloHandler {
    async fn run_async(&mut self) {
        let Some(packet) = self.rx.recv().await else {
            return;
        };
        //todo 配置验证
        let mut iface = self.iface.write().await;
        iface.neighbors.push(packet.router_id);
        iface.neighbors.sort();
        iface.neighbors.dedup();
        //todo state change
    }
}
