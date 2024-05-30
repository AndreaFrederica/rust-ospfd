pub mod ack;
pub mod dd;
pub mod hello;
pub mod lsr;
pub mod lsu;

use std::sync::Arc;

use ospf_packet::{
    packet::{types::*, *},
    Ospf,
};
use tokio::sync::{mpsc, RwLock};

use crate::capture::OspfHandler;
use crate::constant::{AllDRouters, BackboneArea};
use crate::interface::Interface;
use crate::log_error;
use crate::router::RType;

#[derive(Clone)]
pub struct PacketSender {
    pub hello_tx: mpsc::Sender<AddressedHelloPacket>,
    pub dd_tx: mpsc::Sender<AddressedDBDescription>,
    pub lsr_tx: mpsc::Sender<AddressedLSRequest>,
    pub lsu_tx: mpsc::Sender<AddressedLSUpdate>,
    pub ack_tx: mpsc::Sender<AddressedLSAcknowledge>,
}

pub struct PacketReceiver {
    pub hello_rx: mpsc::Receiver<AddressedHelloPacket>,
    pub dd_rx: mpsc::Receiver<AddressedDBDescription>,
    pub lsr_rx: mpsc::Receiver<AddressedLSRequest>,
    pub lsu_rx: mpsc::Receiver<AddressedLSUpdate>,
    pub ack_rx: mpsc::Receiver<AddressedLSAcknowledge>,
}

pub fn channel(buffer: usize) -> (PacketSender, PacketReceiver) {
    let (hello_tx, hello_rx) = mpsc::channel(buffer);
    let (dd_tx, dd_rx) = mpsc::channel(buffer);
    let (lsr_tx, lsr_rx) = mpsc::channel(buffer);
    let (lsu_tx, lsu_rx) = mpsc::channel(buffer);
    let (ack_tx, ack_rx) = mpsc::channel(buffer);
    (PacketSender { hello_tx, dd_tx, lsr_tx, lsu_tx, ack_tx }, PacketReceiver { hello_rx, dd_rx, lsr_rx, lsu_rx, ack_rx })
}

#[allow(non_upper_case_globals)]
#[doc = "首先检查 ospf 报头，对于合法报头，发送给对应报文处理器处理"]
pub fn ospf_handler_maker(interface: Arc<RwLock<Interface>>, tx: PacketSender) -> OspfHandler {
    Box::new(move |src, dest, packet| {
        // debug
        #[cfg(debug_assertions)]
        crate::capture::echo_handler(src, dest, packet.to_immutable());
        // the src & dest has already checked
        if !packet.auto_test_checksum() || packet.get_version() != 2 {
            return;
        }
        // dispatch
        let packet: Ospf = packet.into();
        let interface = interface.clone();
        let tx = tx.clone();
        // packet
        let hd = tokio::spawn(async move {
            let interface = interface.read().await;
            let router = interface.router.read().await;
            match packet.area_id {
                x if x == interface.area_id => (),                        // ok
                BackboneArea if router.router_type != RType::Other => (), // ok
                _ => return,                                              // bad area id
            }
            if dest == AllDRouters && router.router_type == RType::Other {
                return;
            } // bad dest
            match packet.au_type {
                0 => (),      // ok
                _ => todo!(), //todo implement other au type
            }
            let payload = &mut packet.payload.as_slice();
            match packet.message_type {
                HELLO_PACKET => tx
                    .hello_tx
                    .send(AddressedHelloPacket::from_payload(src, packet.router_id, payload))
                    .await
                    .unwrap(),
                DB_DESCRIPTION => tx
                    .dd_tx
                    .send(AddressedDBDescription::from_payload(src, packet.router_id, payload))
                    .await
                    .unwrap(),
                LS_REQUEST => tx
                    .lsr_tx
                    .send(AddressedLSRequest::from_payload(src, packet.router_id, payload))
                    .await
                    .unwrap(),
                LS_UPDATE => tx
                    .lsu_tx
                    .send(AddressedLSUpdate::from_payload(src, packet.router_id, payload))
                    .await
                    .unwrap(),
                LS_ACKNOWLEDGE => tx
                    .ack_tx
                    .send(AddressedLSAcknowledge::from_payload(src, packet.router_id, payload))
                    .await
                    .unwrap(),
                _ => return, // bad msg type
            }
        });
        // error
        tokio::spawn(async move {
            if let Err(e) = hd.await {
                log_error!("Error in handle packet: {:?}", e);
            }
        });
    })
}
