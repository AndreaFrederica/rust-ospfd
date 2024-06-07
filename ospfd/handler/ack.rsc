use ospf_packet::packet::LSAcknowledge;

use crate::neighbor::RefNeighbor;

// @define iface src.get_interface()
// @define neighbor src.get_neighbor()

pub async fn handle(mut src: RefNeighbor<'_>, packet: LSAcknowledge) {
    let name = iface.interface_name.to_string();
    todo!("iface: {}, src: {:?}, packet: {:?}", name, neighbor, packet);
}
