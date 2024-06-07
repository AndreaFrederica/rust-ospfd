use std::ops::Deref;

use ospf_packet::packet::DBDescription;

use crate::{
    database::ProtocolDB,
    log_error, must,
    neighbor::{RefNeighbor, DdPacketCache, NeighborEvent, NeighborState, NeighborSubStruct},
};

// @define iface src.get_interface()
// @define neighbor src.get_neighbor()

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
                src.negotiation_done().await;
                neighbor.option = packet.options;
                neighbor.master = true;
                neighbor.dd_seq_num = dd_cache.sequence_number;
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
                log_error!("todo: should resend last packet to master");
            } else {
                // i am master
                return;
            }
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
    log_error!("todo! update lsa database");
    if prev_state.master {
        neighbor.dd_seq_num = dd_cache.sequence_number;
        log_error!("todo! send dd packet to the master");
        if !dd_cache.more {
            src.exchange_done().await;
        }
    } else {
        neighbor.dd_seq_num = prev_state.dd_seq_num + 1;
        log_error!("todo! send dd packet to the slave");
        if !dd_cache.more {
            src.exchange_done().await;
        }
    }
}
