use ospf_packet::packet::LSUpdate;

use crate::neighbor::RefNeighbor;

// @define iface src.get_interface()
// @define neighbor src.get_neighbor()

pub async fn handle(mut src: RefNeighbor<'_>, packet: LSUpdate) {
    let name = iface.interface_name.to_string();
    todo!("iface: {}, src: {:?}, packet: {:?}", name, neighbor, packet);
}
