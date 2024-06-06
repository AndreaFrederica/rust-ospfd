// thanks to https://zhuanlan.zhihu.com/p/677131879

use std::net::{IpAddr, Ipv4Addr};

use ospf_packet::OspfPacket;
use pnet::datalink::Channel::Ethernet; // 导入以太网通道
use pnet::datalink::{self, DataLinkReceiver, NetworkInterface}; // 导入datalink模块中的相关项
use pnet::packet::ethernet::{EtherTypes, EthernetPacket}; // 导入以太网数据包相关项
use pnet::packet::ip::IpNextHeaderProtocols; // 导入IP协议相关项
use pnet::packet::ipv4::Ipv4Packet; // 导入IPv4数据包相关项
use pnet::packet::Packet; // 导入数据包trait

use crate::constant::{AllDRouters, AllSPFRouters};
use crate::daemon::Runnable;
use crate::{log_error, log_success};

#[cfg(debug_assertions)]
use ospf_packet::{packet, FromBuf};
#[cfg(debug_assertions)]
use crate::log;

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Unhandled channel type")]
    BadChannelType,
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Receiver = Box<(dyn DataLinkReceiver + 'static)>;
pub type OspfHandler = Box<dyn FnMut(Ipv4Addr, Ipv4Addr, OspfPacket) + Send>;

#[doc = "CaptureOspfDaemon: OSPF数据包捕获守护协程"]
pub struct CaptureOspfDaemon {
    ips: Vec<Ipv4Addr>,
    receiver: Receiver,
    handler: OspfHandler,
}

impl CaptureOspfDaemon {
    pub fn new(
        iface: &NetworkInterface,
        handler: impl FnMut(Ipv4Addr, Ipv4Addr, OspfPacket) + Send + 'static,
    ) -> Result<Self, ChannelError> {
        let handler = Box::new(handler);

        let ips = iface
            .ips
            .iter()
            .filter_map(|ip| {
                if let IpAddr::V4(ip) = ip.ip() {
                    Some(ip)
                } else {
                    None
                }
            })
            .collect();
        log_success!("listening on {} ({:?})", iface.name, ips);

        let (_, receiver) = match datalink::channel(iface, Default::default()) {
            // 创建数据链路层通道，用于接收和发送数据包
            Ok(Ethernet(tx, rx)) => (tx, rx), // 如果通道类型是以太网通道，则将发送和接收通道分别赋值给_tx和rx
            Ok(_) => return Err(ChannelError::BadChannelType), // 如果是其他类型的通道，抛出错误
            Err(e) => return Err(e.into()),   // 如果创建通道时发生错误，打印错误消息并退出
        };

        Ok(Self {
            ips,
            receiver,
            handler,
        })
    }

    fn check_ip(ips: &Vec<Ipv4Addr>, ip: Ipv4Addr) -> bool {
        ip == AllSPFRouters || ip == AllDRouters || ips.iter().any(|&i| i == ip)
    }

    fn handle_packet(ips: &Vec<Ipv4Addr>, handler: &mut OspfHandler, ethernet: &EthernetPacket) {
        // 对Ipv4的包按层解析
        match ethernet.get_ethertype() {
            EtherTypes::Ipv4 => {
                // 如果是IPv4数据包
                let header = Ipv4Packet::new(ethernet.payload()); // 解析IPv4头部
                if let Some(header) = header {
                    if Self::check_ip(ips, header.get_source()) {
                        return;
                    } // 不能是自己发出的
                    if !Self::check_ip(ips, header.get_destination()) {
                        return;
                    } // 不能是不发给自己的
                    match header.get_next_level_protocol() {
                        IpNextHeaderProtocols::OspfigP => {
                            // 如果是OSPF协议
                            let packet =
                                OspfPacket::new(header.payload()).expect("Bad Ospf Packet");
                            handler(header.get_source(), header.get_destination(), packet);
                        }
                        _ => (), // 忽略其他协议
                    }
                }
            }
            _ => (), // 忽略非IPv4数据包
        }
    }
}

impl Runnable for CaptureOspfDaemon {
    fn run(&mut self) {
        match self.receiver.next() {
            Ok(packet) => {
                let packet = EthernetPacket::new(packet).expect("Bad Ethernet Packet"); // 解析以太网数据包
                Self::handle_packet(&self.ips, &mut self.handler, &packet); // 处理接收到的数据包
            }
            Err(e) => {
                // 如果读取数据包时发生错误，打印错误消息
                log_error!("An error occurred while reading: {}", e);
            }
        }
    }
}

#[allow(unused)]
#[cfg(debug_assertions)]
#[doc = "打印接收到的数据包"]
pub fn echo_handler(source: Ipv4Addr, destination: Ipv4Addr, packet: OspfPacket) {
    let pkg_str = format!(
        "Received OSPF packet ({} -> {}): {}",
        source, destination, packet
    );
    match packet.get_message_type() {
        packet::types::HELLO_PACKET => {
            let hello_packet = packet::HelloPacket::from_buf(&mut packet.payload());
            log!("{pkg_str} Hello packet: {:#?}", hello_packet);
        }
        packet::types::DB_DESCRIPTION => {
            let db_description = packet::DBDescription::from_buf(&mut packet.payload());
            log!("{pkg_str} DB Description packet: {:#?}", db_description);
        }
        packet::types::LS_REQUEST => {
            let ls_request = packet::LSRequest::from_buf(&mut packet.payload());
            log!("{pkg_str} LS Request packet: {:#?}", ls_request);
        }
        packet::types::LS_UPDATE => {
            let ls_update = packet::LSUpdate::from_buf(&mut packet.payload());
            log!("{pkg_str} LS Update packet: {:#?}", ls_update);
        }
        packet::types::LS_ACKNOWLEDGE => {
            let ls_acknowledge = packet::LSAcknowledge::from_buf(&mut packet.payload());
            log!("{pkg_str} LS Acknowledge packet: {:#?}", ls_acknowledge);
        }
        _ => {
            log!("{pkg_str} Unknown packet type");
        }
    }
}
