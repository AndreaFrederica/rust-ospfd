mod area;
mod capture;
mod command;
mod constant;
mod daemon;
mod database;
mod flooding;
mod gen_lsa;
mod handler;
mod interface;
mod logging;
mod neighbor;
mod sender;
mod util;

use std::{io::Write, net::Ipv4Addr, time::Duration};

use constant::BackboneArea;
use daemon::Daemon;
use database::ProtocolDB;
use interface::{AInterface, Interface};
use pnet::datalink::{self, NetworkInterface};

#[tokio::main()]
async fn main() {
    // 初始化 OSPF 数据库，插入 Backbone 区域
    ProtocolDB::get().await.insert_area(BackboneArea).await;
    
    // 筛选可用网络接口
    let interfaces: Vec<_> = datalink::interfaces().iter().filter_map(start).collect();
    if interfaces.is_empty() {
        panic!("No interface is available");
    }
    
    // 初始化数据库并启动接口
    ProtocolDB::init(&interfaces);
    interfaces.iter().for_each(|i| Interface::start(i));

    log!("waiting to start...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 初始化全局 tokio 运行时句柄
    command::RUNTIME.get_or_init(tokio::runtime::Handle::current);

    // 调用使用 Crossterm 实现的交互主循环
    // 此函数内部会启用原始模式并持续读取用户输入
    command::main_loop();
}

fn start(iface: &NetworkInterface) -> Option<AInterface> {
    if iface.ips.iter().find(|i| i.ip() == Ipv4Addr::LOCALHOST).is_some() {
        return None;
    }
    if iface.ips.iter().find(|i| i.is_ipv4()).is_none() {
        log_warning!("The interface {} do NOT have an ipv4 address", iface.name);
        return None;
    }
    let interface = interface::Interface::from(&iface, BackboneArea);
    let ospf_handler = handler::ospf_handler_maker(interface.clone());
    let capture_daemon = capture::CaptureOspfDaemon::new(&iface, ospf_handler).unwrap();
    tokio::spawn(capture_daemon.run_forever());
    Some(interface)
}
