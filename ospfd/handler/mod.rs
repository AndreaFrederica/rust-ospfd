mod ack;
mod dd;
mod hello;
mod lsr;
mod lsu;

use std::{future::Future, sync::Arc};

use bytes::Buf;
use ospf_packet::{
    packet::{types::*, *},
    FromBuf, Ospf,
};
use tokio::sync::RwLock;

use crate::interface::{Interface, NetType};
use crate::log_error;
use crate::{capture::OspfHandler, util::ip2hex};
use crate::{
    constant::{AllDRouters, BackboneArea},
    interface::AInterface,
    neighbor::{ANeighbor, Neighbor},
    util::hex2ip,
};

#[allow(non_upper_case_globals)]
#[doc = "首先检查 ospf 报头，对于合法报头，发送给对应报文处理器处理"]
pub fn ospf_handler_maker(interface: Arc<RwLock<Interface>>) -> OspfHandler {
    Box::new(move |src, dest, packet| {
        // debug
        #[cfg(debug_assertions)]
        crate::log!(
            "packet received: {} ({} bytes)",
            message_type_string(packet.get_message_type()),
            packet.get_length()
        );
        // the src & dest has already checked
        if !packet.auto_test_checksum() || packet.get_version() != 2 {
            return;
        }
        // dispatch
        let packet: Ospf = packet.into();
        let interface = interface.clone();
        // packet
        let hd = tokio::spawn(async move {
            let iface = interface.read().await;
            match packet.area_id {
                x if x == ip2hex(iface.area_id) => (),                 // ok
                BackboneArea if iface.is_dr() || iface.is_bdr() => (), // ok
                _ => return,                                           // bad area id
            }
            if dest == AllDRouters && iface.is_drother() {
                return;
            } // bad dest
            match packet.au_type {
                0 => (),      // ok
                _ => todo!(), //todo implement other au type
            }
            let payload = &mut packet.payload.as_slice();
            let mut router_id = hex2ip(packet.router_id);
            let mut ip = src;
            if matches!(iface.net_type, NetType::P2P | NetType::Virtual) {
                std::mem::swap(&mut router_id, &mut ip);
            }
            let neighbor = interface
                .read()
                .await
                .get_neighbor(ip)
                .await
                .unwrap_or(Neighbor::new(&interface, router_id, ip));
            drop(iface); // release the lock
            match packet.message_type {
                HELLO_PACKET => handle::<HelloPacket>(interface, neighbor, payload).await,
                DB_DESCRIPTION => handle::<DBDescription>(interface, neighbor, payload).await,
                LS_REQUEST => handle::<LSRequest>(interface, neighbor, payload).await,
                LS_UPDATE => handle::<LSUpdate>(interface, neighbor, payload).await,
                LS_ACKNOWLEDGE => handle::<LSAcknowledge>(interface, neighbor, payload).await,
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

trait HandlePacket {
    async fn handle(self, iface: AInterface, src: ANeighbor);
}

impl HandlePacket for LSAcknowledge {
    fn handle(self, iface: AInterface, src: ANeighbor) -> impl Future<Output = ()> {
        ack::handle(iface, src, self)
    }
}

impl HandlePacket for DBDescription {
    fn handle(self, iface: AInterface, src: ANeighbor) -> impl Future<Output = ()> {
        dd::handle(iface, src, self)
    }
}

impl HandlePacket for HelloPacket {
    fn handle(self, iface: AInterface, src: ANeighbor) -> impl Future<Output = ()> {
        hello::handle(iface, src, self)
    }
}

impl HandlePacket for LSRequest {
    fn handle(self, iface: AInterface, src: ANeighbor) -> impl Future<Output = ()> {
        lsr::handle(iface, src, self)
    }
}

impl HandlePacket for LSUpdate {
    fn handle(self, iface: AInterface, src: ANeighbor) -> impl Future<Output = ()> {
        lsu::handle(iface, src, self)
    }
}

fn handle<T: FromBuf + HandlePacket>(
    iface: AInterface,
    src: ANeighbor,
    payload: &mut impl Buf,
) -> impl Future<Output = ()> {
    let packet = T::from_buf(payload);
    packet.handle(iface, src)
}
