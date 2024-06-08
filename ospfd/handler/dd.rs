use std::{marker::PhantomData, ops::Deref};

use ospf_macros::define;
use ospf_packet::{lsa, packet::DBDescription};

use crate::{
    database::ProtocolDB,
    guard, must,
    neighbor::{
        DdPacketCache, DdRxmt, NeighborEvent, NeighborState, NeighborSubStruct, RefNeighbor,
    },
    sender::send_packet,
};

#[define(iface => src.get_interface(); neighbor => src.get_neighbor())]
pub async fn handle(mut src: RefNeighbor<'_>, packet: DBDescription) {
    must!(neighbor.state >= NeighborState::Init);
    if neighbor.state == NeighborState::Init {
        src.two_way_received().await;
    }
    let dd_cache = DdPacketCache::from(&packet);
    let prev_state = NeighborSubStruct::from(neighbor.deref());
    neighbor.dd_last_packet = dd_cache;
    match prev_state.state {
        NeighborState::ExStart => {
            if dd_cache.init
                && dd_cache.more
                && dd_cache.master
                && packet.lsa_header.is_empty()
                && prev_state.router_id > ProtocolDB::get().router_id
            {
                // 设定了I,M,MS选项位，包的其他部分为空，且邻居路由器标识比自身路由器标识要大
                neighbor.option = packet.options;
                neighbor.master = true;
                neighbor.dd_seq_num = dd_cache.sequence_number;
                src.negotiation_done().await;
            } else if !dd_cache.init
                && !dd_cache.master
                && dd_cache.sequence_number == prev_state.dd_seq_num
                && prev_state.router_id < ProtocolDB::get().router_id
            {
                // 清除了I,MS选项位，且包中的 DD 序号等于邻居数据结构中的 DD 序号（标明为确认）
                // 而且邻居路由器标识比自身路由器标识要小
                src.negotiation_done().await;
                neighbor.master = false;
            }
        }
        NeighborState::Exchange | NeighborState::Loading | NeighborState::Full
            if prev_state.dd_last_packet == dd_cache =>
        {
            // 重复 DD 包
            if prev_state.master {
                // i am slave
                guard! {
                    DdRxmt::Packet(ref p) = neighbor.dd_rxmt;
                    error: "There are no dd packet to resend to the master({})!", neighbor.router_id
                };
                let packet = p.clone();
                let ip = neighbor.ip_addr;
                send_packet(iface, &packet, ip).await;
            }
            return;
        }
        NeighborState::Exchange => {
            // 主从（MS）位的状态与当前的主从关系不匹配
            // 意外设定了初始（I）位
            // OSPF 可选项不同
            // seq number 不匹配
            if dd_cache.master != prev_state.master
                || dd_cache.init
                || packet.options != prev_state.option
                || dd_cache.master && dd_cache.sequence_number != prev_state.dd_seq_num + 1
                || !dd_cache.master && dd_cache.sequence_number != prev_state.dd_seq_num
            {
                src.seq_number_mismatch().await;
                return;
            }
        }
        NeighborState::Loading | NeighborState::Full => {
            // 在此状态时，路由器已经收发了全部 DD 包。只可能接收重复的 DD 包
            src.seq_number_mismatch().await;
            return;
        }
        _ => return,
    }
    // update database
    for ref lsa in packet.lsa_header {
        must!((1..=5).contains(&lsa.ls_type); else: src.seq_number_mismatch().await);
        must!(lsa.ls_type != lsa::types::AS_EXTERNAL_LSA || iface.external_routing; else: src.seq_number_mismatch().await);
        if ProtocolDB::get().need_update(iface.area_id, lsa).await {
            neighbor.ls_request_list.push_back(lsa.clone());
        }
    }
    src.spawn_lsr_sender();
    // send dd
    if neighbor.master {
        // send dd to master
        neighbor.dd_seq_num = dd_cache.sequence_number;
        let ip = neighbor.ip_addr;
        let more = dd_cache.more as u8;
        let mut len = neighbor.db_summary_list.len();
        if dd_cache.more {
            len = len.min(8);
        }
        let packet = DBDescription {
            interface_mtu: 0,
            options: neighbor.option,
            _zeros: PhantomData,
            init: 0,
            more,
            master: 0,
            db_sequence_number: neighbor.dd_seq_num,
            lsa_header: neighbor.db_summary_list.drain(0..len).collect(),
        };
        send_packet(iface, &packet, ip).await;
        neighbor.dd_rxmt.set(DdRxmt::Packet(packet));
        must!(dd_cache.more; else: src.exchange_done().await);
    } else {
        // send dd to slave
        neighbor.dd_seq_num = prev_state.dd_seq_num + 1;
        must!(dd_cache.more; else: src.exchange_done().await);
        let len = neighbor.db_summary_list.len().min(12);
        let packet = DBDescription {
            interface_mtu: 0,
            options: neighbor.option,
            _zeros: PhantomData,
            init: 0,
            more: (len < neighbor.db_summary_list.len()) as u8,
            master: 1,
            db_sequence_number: neighbor.dd_seq_num,
            lsa_header: neighbor.db_summary_list.drain(0..len).collect(),
        };
        src.spawn_master_send_dd(packet);
    }
}
