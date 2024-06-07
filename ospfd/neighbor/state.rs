use std::ops::Deref;

use futures::executor;

use super::{ANeighbor, Neighbor};
use crate::{interface::NetType, log_error, log_success, must};

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
    async fn hello_receive(self);
    async fn start(self);
    async fn two_way_received(self);
    async fn negotiation_done(self);
    async fn exchange_done(self);
    async fn bad_ls_req(self);
    async fn loading_done(self);
    async fn adj_ok(self);
    async fn seq_number_mismatch(self);
    async fn one_way_received(self);
    async fn kill_nbr(self);
    async fn inactivity_timer(self);
    async fn ll_down(self);
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
        "neighbor {}'s state changed: {:?} -> {:?}",
        neighbor.router_id,
        old,
        neighbor.state
    );
}

impl NeighborEvent for ANeighbor {
    async fn hello_receive(self) {
        #[cfg(debug_assertions)]
        log_event("hello_receive", self.read().await.deref());
        let old = self.read().await.state;
        if old <= NeighborState::Attempt {
            self.write().await.state = NeighborState::Init;
        }
        reset_timer(self.clone()).await;
        log_state(old, self.read().await.deref());
    }

    async fn start(self) {
        #[cfg(debug_assertions)]
        log_event("start", self.read().await.deref());
        //todo with NBMA
        todo!("start NBMA")
    }

    async fn two_way_received(self) {
        #[cfg(debug_assertions)]
        log_event("two_way_received", self.read().await.deref());
        let old = self.read().await.state;
        must!(old == NeighborState::Init);
        let mut this = self.write().await;
        this.state = if judge_connect(&this).await {
            NeighborState::ExStart
        } else {
            NeighborState::TwoWay
        };
        drop(this);
        ex_start(self.clone()).await;
        log_state(old, self.read().await.deref());
    }

    async fn negotiation_done(self) {
        #[cfg(debug_assertions)]
        log_event("negotiation_done", self.read().await.deref());
        must!(self.read().await.state == NeighborState::ExStart);
        //todo summary lsa
        log_error!("todo! negotiation_done");
        self.write().await.state = NeighborState::Exchange;
        log_state(NeighborState::ExStart, self.read().await.deref());
    }

    async fn exchange_done(self) {
        #[cfg(debug_assertions)]
        log_event("exchange_done", self.read().await.deref());
        must!(self.read().await.state == NeighborState::Exchange);
        if self.read().await.ls_request_list.is_empty() {
            self.write().await.state = NeighborState::Full;
        } else {
            self.write().await.state = NeighborState::Loading;
            //todo send ls request
            //todo after receive ls update, call loading_done
            log_error!("todo! send ls request");
        }
        log_state(NeighborState::Exchange, self.read().await.deref());
    }

    async fn bad_ls_req(self) {
        #[cfg(debug_assertions)]
        log_event("bad_ls_req", self.read().await.deref());
        let old = self.read().await.state;
        must!(old >= NeighborState::Exchange);
        self.write().await.reset();
        self.write().await.state = NeighborState::ExStart;
        ex_start(self.clone()).await;
        log_state(old, self.read().await.deref());
    }

    async fn loading_done(self) {
        #[cfg(debug_assertions)]
        log_event("loading_done", self.read().await.deref());
        must!(self.read().await.state == NeighborState::Loading);
        self.write().await.state = NeighborState::Full;
        log_state(NeighborState::Loading, self.read().await.deref());
    }

    async fn adj_ok(self) {
        #[cfg(debug_assertions)]
        log_event("adj_ok", self.read().await.deref());
        let old = self.read().await.state;
        if old == NeighborState::TwoWay {
            let mut this = self.write().await;
            this.state = if judge_connect(&this).await {
                NeighborState::ExStart
            } else {
                NeighborState::TwoWay
            };
            drop(this);
            ex_start(self.clone()).await;
        } else if old >= NeighborState::ExStart {
            let mut this = self.write().await;
            if !judge_connect(&this).await {
                this.state = NeighborState::TwoWay;
                this.reset();
            }
        }
        log_state(old, self.read().await.deref());
    }

    async fn seq_number_mismatch(self) {
        #[cfg(debug_assertions)]
        log_event("seq_number_mismatch", self.read().await.deref());
        let old = self.read().await.state;
        must!(old >= NeighborState::Exchange);
        self.write().await.reset();
        self.write().await.state = NeighborState::ExStart;
        ex_start(self.clone()).await;
        log_state(old, self.read().await.deref());
    }

    async fn one_way_received(self) {
        #[cfg(debug_assertions)]
        log_event("one_way_received", self.read().await.deref());
        let old = self.read().await.state;
        must!(old >= NeighborState::TwoWay);
        self.write().await.reset();
        self.write().await.state = NeighborState::Init;
        log_state(old, self.read().await.deref());
    }

    async fn kill_nbr(self) {
        #[cfg(debug_assertions)]
        log_event("kill_nbr", self.read().await.deref());
        let old = self.read().await.state;
        self.write().await.reset();
        self.write().await.inactive_timer.abort();
        self.write().await.state = NeighborState::Down;
        log_state(old, self.read().await.deref());
    }

    async fn inactivity_timer(self) {
        #[cfg(debug_assertions)]
        log_event("inactivity_timer", self.read().await.deref());
        let old = self.read().await.state;
        self.write().await.reset();
        self.write().await.state = NeighborState::Down;
        log_state(old, self.read().await.deref());
    }

    async fn ll_down(self) {
        #[cfg(debug_assertions)]
        log_event("ll_down", self.read().await.deref());
        let old = self.read().await.state;
        self.write().await.reset();
        self.write().await.inactive_timer.abort();
        self.write().await.state = NeighborState::Down;
        log_state(old, self.read().await.deref());
    }
}

async fn reset_timer(this: ANeighbor) {
    let dead_interval = this
        .read()
        .await
        .interface
        .upgrade()
        .map(|i| executor::block_on(i.read()).dead_interval)
        .unwrap_or(1) as u64;
    let cloned = this.clone();
    let mut this = this.write().await;
    this.inactive_timer.abort();
    this.inactive_timer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(dead_interval)).await;
        cloned.inactivity_timer().await;
    });
}

async fn judge_connect(this: &Neighbor) -> bool {
    let Some(interface) = this.interface.upgrade() else {
        return false;
    };
    let interface = interface.read().await;
    matches!(
        interface.net_type,
        NetType::P2P | NetType::P2MP | NetType::Virtual
    ) || interface.is_bdr()
        || interface.is_dr()
        || this.is_bdr()
        || this.is_dr()
}

async fn ex_start(this: ANeighbor) {
    let mut this = this.write().await;
    if !(this.state == NeighborState::ExStart && this.dd_seq_num == 0) {
        return;
    }
    // first time
    this.dd_seq_num = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    this.master = false;
    //todo begin sending dd packet...
    log_error!("todo! send dd packet")
}
