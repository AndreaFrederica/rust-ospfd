mod ack;
mod dd;
mod hello;
mod lsr;
mod lsu;

use std::{future::Future, net::Ipv4Addr, ops::DerefMut};

use bytes::Buf;
use ospf_packet::{
    packet::{types::*, *},
    FromBuf, Ospf,
};

use crate::{
    capture::OspfHandler,
    constant::AllDRouters,
    interface::{AInterface, NetType},
    log_error,
    neighbor::{Neighbor, RefNeighbor},
    util::{hex2ip, ip2hex},
};

#[allow(non_upper_case_globals)]
#[doc = "首先检查 ospf 报头，对于合法报头，发送给对应报文处理器处理"]
pub fn ospf_handler_maker(interface: AInterface) -> OspfHandler {
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
        let hd = tokio::spawn(ospf_handle(interface.clone(), packet.into(), src, dest));
        // error
        tokio::spawn(async move {
            if let Err(e) = hd.await {
                log_error!("Error in handle packet: {:?}", e);
            }
        });
    })
}

async fn ospf_handle(interface: AInterface, packet: Ospf, src: Ipv4Addr, dest: Ipv4Addr) {
    let mut interface = interface.write().await;
    match packet.area_id {
        x if x == ip2hex(interface.area_id) => (),          // ok
        0 if interface.is_dr() || interface.is_bdr() => (), // ok
        _ => return,                                        // bad area id
    }
    if dest == AllDRouters && interface.is_drother() {
        return;
    } // bad dest
    match packet.au_type {
        0 => (),      // ok
        _ => todo!(), //todo implement other au type
    }
    let payload = &mut packet.payload.as_slice();
    let mut router_id = hex2ip(packet.router_id);
    let mut ip = src;
    if matches!(interface.net_type, NetType::P2P | NetType::Virtual) {
        std::mem::swap(&mut router_id, &mut ip);
    }
    // insert neighbor
    if !interface.neighbors.contains_key(&ip) {
        interface.neighbors.insert(ip, Neighbor::new(router_id, ip));
    }
    let neighbor = RefNeighbor::from(interface.deref_mut(), ip).unwrap();
    match packet.message_type {
        HELLO_PACKET => handle::<HelloPacket>(neighbor, payload).await,
        DB_DESCRIPTION => handle::<DBDescription>(neighbor, payload).await,
        LS_REQUEST => handle::<LSRequest>(neighbor, payload).await,
        LS_UPDATE => handle::<LSUpdate>(neighbor, payload).await,
        LS_ACKNOWLEDGE => handle::<LSAcknowledge>(neighbor, payload).await,
        _ => return, // bad msg type
    }
}

trait HandlePacket {
    async fn handle(self, src: RefNeighbor);
}

impl HandlePacket for LSAcknowledge {
    fn handle(self, src: RefNeighbor) -> impl Future<Output = ()> {
        ack::handle(src, self)
    }
}

impl HandlePacket for DBDescription {
    fn handle(self, src: RefNeighbor) -> impl Future<Output = ()> {
        dd::handle(src, self)
    }
}

impl HandlePacket for HelloPacket {
    fn handle(self, src: RefNeighbor) -> impl Future<Output = ()> {
        hello::handle(src, self)
    }
}

impl HandlePacket for LSRequest {
    fn handle(self, src: RefNeighbor) -> impl Future<Output = ()> {
        lsr::handle(src, self)
    }
}

impl HandlePacket for LSUpdate {
    fn handle(self, src: RefNeighbor) -> impl Future<Output = ()> {
        lsu::handle(src, self)
    }
}

async fn handle<T>(src: RefNeighbor<'_>, payload: &mut impl Buf)
where
    T: FromBuf + HandlePacket,
{
    let packet = T::from_buf(payload);
    packet.handle(src).await
}
