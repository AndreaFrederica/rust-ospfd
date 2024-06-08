use std::{marker::PhantomData, ops::DerefMut};

use ospf_packet::packet::DBDescription;

use super::{Neighbor, RefNeighbor};
use crate::{database::ProtocolDB, guard, interface::NetType, log_error, log_success, must};

#[cfg(debug_assertions)]
use crate::log;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NeighborState {
    Down,
    Attempt,
    Init,
    TwoWay,
    ExStart,
    Exchange,
    Loading,
    Full,
}

// helper trait for event handling
#[allow(unused)]
pub trait NeighborEvent: Send {
    async fn hello_receive(&mut self);
    async fn start(&mut self);
    async fn two_way_received(&mut self);
    async fn negotiation_done(&mut self);
    async fn exchange_done(&mut self);
    async fn bad_ls_req(&mut self);
    async fn loading_done(&mut self);
    async fn adj_ok(&mut self);
    async fn seq_number_mismatch(&mut self);
    async fn one_way_received(&mut self);
    async fn kill_nbr(&mut self);
    async fn inactivity_timer(&mut self);
    async fn ll_down(&mut self);
}

#[cfg(debug_assertions)]
fn log_event(event: &str, neighbor: &Neighbor) {
    log!(
        "neighbor {}({:?}) recv event: {}",
        neighbor.router_id,
        neighbor.state,
        event
    );
}

fn log_state(old: NeighborState, neighbor: &Neighbor) {
    if old == neighbor.state {
        return;
    }
    log_success!(
        "neighbor {}({})'s state changed: {:?} -> {:?}",
        neighbor.router_id,
        if neighbor.master { "master" } else { "slave" },
        old,
        neighbor.state
    );
}

impl NeighborEvent for RefNeighbor<'_> {
    async fn hello_receive(&mut self) {
        #[cfg(debug_assertions)]
        log_event("hello_receive", self.get_neighbor());
        let old = self.get_neighbor().state;
        if old <= NeighborState::Attempt {
            self.get_neighbor().state = NeighborState::Init;
        }
        reset_timer(self).await;
        log_state(old, self.get_neighbor());
    }

    async fn start(&mut self) {
        #[cfg(debug_assertions)]
        log_event("start", self.get_neighbor());
        //todo with NBMA
        todo!("start NBMA")
    }

    async fn two_way_received(&mut self) {
        #[cfg(debug_assertions)]
        log_event("two_way_received", self.get_neighbor());
        let old = self.get_neighbor().state;
        must!(old == NeighborState::Init);
        self.get_neighbor().state = if judge_connect(self).await {
            NeighborState::ExStart
        } else {
            NeighborState::TwoWay
        };
        ex_start(self);
        log_state(old, self.get_neighbor());
    }

    async fn negotiation_done(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("negotiation_done", this);
        must!(this.state == NeighborState::ExStart);
        summary_lsa(self).await;
        self.get_neighbor().state = NeighborState::Exchange;
        log_state(NeighborState::ExStart, self.get_neighbor());
    }

    async fn exchange_done(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("exchange_done", this);
        must!(this.state == NeighborState::Exchange);
        this.dd_rxmt.reset();
        if this.ls_request_list.is_empty() {
            this.state = NeighborState::Full;
        } else {
            this.state = NeighborState::Loading;
            //todo send ls request
            //todo after receive ls update, call loading_done
            log_error!("todo! send ls request");
        }
        log_state(NeighborState::Exchange, this);
    }

    async fn bad_ls_req(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("bad_ls_req", this);
        let old = this.state;
        must!(old >= NeighborState::Exchange);
        this.reset();
        this.state = NeighborState::ExStart;
        ex_start(self);
        log_state(old, self.get_neighbor());
    }

    async fn loading_done(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("loading_done", this);
        must!(this.state == NeighborState::Loading);
        this.state = NeighborState::Full;
        log_state(NeighborState::Loading, this);
    }

    async fn adj_ok(&mut self) {
        #[cfg(debug_assertions)]
        log_event("adj_ok", self.get_neighbor());
        let old = self.get_neighbor().state;
        if old == NeighborState::TwoWay {
            self.get_neighbor().state = if judge_connect(self).await {
                NeighborState::ExStart
            } else {
                NeighborState::TwoWay
            };
            ex_start(self);
        } else if old >= NeighborState::ExStart {
            if !judge_connect(self).await {
                self.get_neighbor().state = NeighborState::TwoWay;
                self.get_neighbor().reset();
            }
        }
        log_state(old, self.get_neighbor());
    }

    async fn seq_number_mismatch(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("seq_number_mismatch", this);
        let old = this.state;
        must!(old >= NeighborState::Exchange);
        this.reset();
        this.state = NeighborState::ExStart;
        ex_start(self);
        log_state(old, self.get_neighbor());
    }

    async fn one_way_received(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("one_way_received", this);
        let old = this.state;
        must!(old >= NeighborState::TwoWay);
        this.reset();
        this.state = NeighborState::Init;
        log_state(old, this);
    }

    async fn kill_nbr(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("kill_nbr", this);
        let old = this.state;
        this.reset();
        this.inactive_timer.abort();
        this.state = NeighborState::Down;
        log_state(old, this);
    }

    async fn inactivity_timer(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("inactivity_timer", this);
        let old = this.state;
        this.reset();
        this.state = NeighborState::Down;
        log_state(old, this);
    }

    async fn ll_down(&mut self) {
        let this = self.get_neighbor();
        #[cfg(debug_assertions)]
        log_event("ll_down", this);
        let old = this.state;
        this.reset();
        this.inactive_timer.abort();
        this.state = NeighborState::Down;
        log_state(old, this);
    }
}

async fn reset_timer(this: &mut RefNeighbor<'_>) {
    let dead_interval = this.get_interface().dead_interval as u64;
    let iface = this.get_interface().me.clone();
    let this = this.get_neighbor();
    let ip = this.ip_addr;
    this.inactive_timer.abort();
    this.inactive_timer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(dead_interval)).await;
        guard!(Some(iface) = iface.upgrade());
        let mut iface = iface.lock().await;
        RefNeighbor::from(iface.deref_mut(), ip)
            .unwrap()
            .inactivity_timer()
            .await;
    });
}

async fn judge_connect(this: &mut RefNeighbor<'_>) -> bool {
    matches!(
        this.get_interface().net_type,
        NetType::P2P | NetType::P2MP | NetType::Virtual
    ) || this.get_interface().is_bdr()
        || this.get_interface().is_dr()
        || this.get_neighbor().is_bdr()
        || this.get_neighbor().is_dr()
}

fn ex_start(this: &mut RefNeighbor<'_>) {
    let neighbor = this.get_neighbor();
    must!(neighbor.state == NeighborState::ExStart && neighbor.dd_seq_num == 0);
    // first time
    neighbor.dd_seq_num = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u16 as u32;
    neighbor.master = false;
    let packet = DBDescription {
        interface_mtu: 0,
        options: this.get_neighbor().option,
        _zeros: PhantomData,
        init: 1,
        more: 1,
        master: 1,
        db_sequence_number: this.get_neighbor().dd_seq_num,
        lsa_header: vec![],
    };
    this.spawn_master_send_dd(packet);
}

pub async fn summary_lsa(this: &mut RefNeighbor<'_>) {
    let areas = ProtocolDB::get().areas.lock().await;
    guard! {
        Some(area) = areas.get(&this.get_interface().area_id);
        error: "Area({}) not found in database!", this.get_interface().area_id
    };
    this.get_neighbor()
        .db_summary_list
        .extend(area.router_lsa.values().map(|v| v.0.clone()));
    this.get_neighbor()
        .db_summary_list
        .extend(area.network_lsa.values().map(|v| v.0.clone()));
    this.get_neighbor()
        .db_summary_list
        .extend(area.ip_summary_lsa.values().map(|v| v.0.clone()));
    this.get_neighbor()
        .db_summary_list
        .extend(area.asbr_summary_lsa.values().map(|v| v.0.clone()));
    if this.get_interface().external_routing {
        this.get_neighbor().db_summary_list.extend(
            ProtocolDB::get()
                .as_external_lsa
                .lock()
                .await
                .values()
                .map(|v| v.0.clone()),
        );
    }
}
