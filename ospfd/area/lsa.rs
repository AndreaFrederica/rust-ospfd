use std::{
    future::Future,
    time::{Duration, Instant},
};

use ospf_packet::lsa::{Lsa, LsaHeader};
use tokio::task::AbortHandle;

use crate::constant::LsaMaxAge;

pub struct LsaTimer {
    created: Instant,
    refresh: AbortHandle,
}

impl Drop for LsaTimer {
    fn drop(&mut self) {
        self.refresh.abort();
    }
}

impl LsaTimer {
    pub fn new<F>(refresh_time: u64, refresh_handle: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self {
            created: Instant::now(),
            refresh: tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(refresh_time)).await;
                refresh_handle.await;
            })
            .abort_handle(),
        }
    }

    pub fn update_lsa_age(&self, mut lsa: Lsa) -> Lsa {
        let age = lsa.header.ls_age as u64 + self.created.elapsed().as_secs();
        lsa.header.ls_age = age.max(LsaMaxAge as u64) as u16;
        lsa
    }

    pub fn update_lsa_age_header(&self, mut header: LsaHeader) -> LsaHeader {
        let age = header.ls_age as u64 + self.created.elapsed().as_secs();
        header.ls_age = age.max(LsaMaxAge as u64) as u16;
        header
    }

    pub fn get_created(&self) -> Instant {
        self.created
    }
}
