/// 添加一个 LSA 到路由器数据库：a）在洪泛过程中接收（见第 13 章）；b）路由器自己生成
/// （见第 12.4 节）。从路由器数据库删除一个 LSA：a）在洪泛过程中被较新的实例所覆盖（见第
/// 13 章）；b）路由器自己生成了较新的实例（见第 12.4 节）；c）LSA 超时，并从路由域中被废
/// 止（见第 14 章）。当从数据库中删除 LSA 时，必须将其同时从所有的邻居连接状态重传列表
/// 中删除（见第 10 章）。

use std::{
    fmt::Debug, future::Future, time::{Duration, Instant}
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
        lsa.header = self.update_lsa_age_header(lsa.header);
        lsa
    }

    pub fn update_lsa_age_header(&self, mut header: LsaHeader) -> LsaHeader {
        let age = header.ls_age as u64 + self.created.elapsed().as_secs();
        header.ls_age = age.min(LsaMaxAge as u64) as u16;
        header
    }

    pub fn get_created(&self) -> Instant {
        self.created
    }
}

impl Debug for LsaTimer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.created.fmt(f)
    }
}
