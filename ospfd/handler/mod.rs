mod ack;
mod dd;
mod flooding;
mod hello;
mod lsr;
mod lsu;

use std::{net::Ipv4Addr, ops::DerefMut};

use ospf_packet::{
    packet::{types::*, *},
    FromBuf, Ospf,
};

use crate::{
    capture::OspfHandler,
    constant::AllDRouters,
    database::ProtocolDB,
    interface::{AInterface, NetType},
    log_error,
    neighbor::{Neighbor, RefNeighbor},
    util::{hex2ip, ip2hex},
};

#[allow(non_upper_case_globals)]
#[doc = "首先检查 ospf 报头，对于合法报头，发送给对应报文处理器处理"]
pub fn ospf_handler_maker(interface: AInterface) -> OspfHandler {
    Box::new(move |src, dest, packet| {
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
    let mut interface = interface.lock().await;
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
        HELLO_PACKET => hello::handle(neighbor, HelloPacket::from_buf(payload)).await,
        DB_DESCRIPTION => dd::handle(neighbor, DBDescription::from_buf(payload)).await,
        LS_REQUEST => lsr::handle(neighbor, LSRequest::from_buf(payload)).await,
        LS_UPDATE => {
            lsu::handle(
                ProtocolDB::upgrade_lock(interface).await,
                ip,
                LSUpdate::from_buf(payload),
            )
            .await
        }
        LS_ACKNOWLEDGE => ack::handle(neighbor, LSAcknowledge::from_buf(payload)).await,
        _ => return, // bad msg type
    }
}
