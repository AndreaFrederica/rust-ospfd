// thanks to https://zhuanlan.zhihu.com/p/677131879

use ospf_packet::{packet, FromBuf};
use pnet::datalink::Channel::Ethernet; // 导入以太网通道
use pnet::datalink::{self, NetworkInterface}; // 导入datalink模块中的相关项
use pnet::packet::ethernet::{EtherTypes, EthernetPacket}; // 导入以太网数据包相关项
use pnet::packet::ip::IpNextHeaderProtocols; // 导入IP协议相关项
use pnet::packet::ipv4::Ipv4Packet; // 导入IPv4数据包相关项
use pnet::packet::Packet; // 导入数据包trait

fn handle_packet(ethernet: &EthernetPacket) {
    // 对Ipv4的包按层解析
    match ethernet.get_ethertype() {
        EtherTypes::Ipv4 => {
            // 如果是IPv4数据包
            let header = Ipv4Packet::new(ethernet.payload()); // 解析IPv4头部
            if let Some(header) = header {
                match header.get_next_level_protocol() {
                    IpNextHeaderProtocols::OspfigP => {
                        // 如果是OSPF协议
                        let packet = ospf_packet::OspfPacket::new(header.payload()).unwrap(); // 解析OSPF数据包
                        println!(
                            "Received OSPF packet ({} -> {}): {:?}",
                            header.get_source(),
                            header.get_destination(),
                            packet
                        ); // 打印接收到的OSPF数据包
                        match packet.get_message_type() {
                            packet::types::HELLO_PACKET => {
                                let hello_packet = packet::HelloPacket::from_buf(&mut packet.payload());
                                println!("> Hello packet: {:?}", hello_packet);
                            }
                            packet::types::DB_DESCRIPTION => {
                                let db_description = packet::DBDescription::from_buf(&mut packet.payload());
                                println!("> DB Description packet: {:?}", db_description);
                            }
                            packet::types::LS_REQUEST => {
                                let ls_request = packet::LSRequest::from_buf(&mut packet.payload());
                                println!("> LS Request packet: {:?}", ls_request);
                            }
                            packet::types::LS_UPDATE => {
                                let ls_update = packet::LSUpdate::from_buf(&mut packet.payload());
                                println!("> LS Update packet: {:?}", ls_update);
                            }
                            packet::types::LS_ACKNOWLEDGE => {
                                let ls_acknowledge = packet::LSAcknowledge::from_buf(&mut packet.payload());
                                println!("> LS Acknowledge packet: {:?}", ls_acknowledge);
                            }
                            _ => {
                                println!("> Unknown packet type");
                            }
                        }
                        println!("");
                    }
                    _ => println!("Ignoring non OSPF packet"), // 忽略其他非TCP协议
                }
            }
        }
        _ => println!("Ignoring non IPv4 packet"), // 忽略非IPv4数据包
    }
}

pub async fn capture(interface_name: String) {
    // 获取网卡列表
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(|iface: &NetworkInterface| iface.name == interface_name) // 根据接口名称过滤网卡列表
        .next()
        .expect("Error getting interface"); // 如果找不到匹配的接口，打印错误消息并退出

    let (_tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        // 创建数据链路层通道，用于接收和发送数据包
        Ok(Ethernet(tx, rx)) => (tx, rx), // 如果通道类型是以太网通道，则将发送和接收通道分别赋值给_tx和rx
        Ok(_) => panic!("Unhandled channel type"), // 如果是其他类型的通道，抛出错误
        Err(e) => panic!(
            "An error occurred when creating the datalink channel: {}",
            e
        ), // 如果创建通道时发生错误，打印错误消息并退出
    };

    loop {
        // 获取收到的包
        match rx.next() {
            Ok(packet) => {
                let packet = EthernetPacket::new(packet).unwrap(); // 解析以太网数据包
                handle_packet(&packet); // 处理接收到的数据包
            }
            Err(e) => {
                panic!("An error occurred while reading: {}", e); // 如果读取数据包时发生错误，打印错误消息并退出
            }
        }
    }
}
