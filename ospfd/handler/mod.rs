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

use crate::capture::{echo_handler, OspfHandler};
use crate::constant::{AllDRouters, BackboneArea};
use crate::interface::Interface;
use crate::log_error;
use crate::router::RType;

#[allow(non_upper_case_globals)]
#[doc = "首先检查 ospf 报头，对于合法报头，发送给对应报文处理器处理"]
pub fn ospf_handler_maker(
    interface: Arc<RwLock<Interface>>,
    hello_tx: mpsc::Sender<AddressedHelloPacket>,
    dd_tx: mpsc::Sender<AddressedDBDescription>,
    lsr_tx: mpsc::Sender<AddressedLSRequest>,
    lsu_tx: mpsc::Sender<AddressedLSUpdate>,
    ack_tx: mpsc::Sender<AddressedLSAcknowledge>,
) -> OspfHandler {
    Box::new(move |src, dest, packet| {
        // debug
        #[cfg(debug_assertions)]
        echo_handler(src, dest, packet.to_immutable());
        // the src & dest has already checked
        if !packet.auto_test_checksum() || packet.get_version() != 2 {
            return;
        }
        // dispatch
        let packet: Ospf = packet.into();
        let interface = interface.clone();
        let hello_tx = hello_tx.clone();
        let dd_tx = dd_tx.clone();
        let lsr_tx = lsr_tx.clone();
        let lsu_tx = lsu_tx.clone();
        let ack_tx = ack_tx.clone();
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
                HELLO_PACKET => hello_tx
                    .send(AddressedHelloPacket::from_payload(src, dest, payload))
                    .await
                    .unwrap(),
                DB_DESCRIPTION => dd_tx
                    .send(AddressedDBDescription::from_payload(src, dest, payload))
                    .await
                    .unwrap(),
                LS_REQUEST => lsr_tx
                    .send(AddressedLSRequest::from_payload(src, dest, payload))
                    .await
                    .unwrap(),
                LS_UPDATE => lsu_tx
                    .send(AddressedLSUpdate::from_payload(src, dest, payload))
                    .await
                    .unwrap(),
                LS_ACKNOWLEDGE => ack_tx
                    .send(AddressedLSAcknowledge::from_payload(src, dest, payload))
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
