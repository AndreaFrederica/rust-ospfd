use std::net::Ipv4Addr;

use crate::interface::Interface;

use super::Neighbor;

/// I think RefNeighbor is 100% safe, because it is impossible to borrow interface elsewhere
/// and it is impossible to borrow both neighbor and interface at one time.
pub struct RefNeighbor<'a> {
    interface: &'a mut Interface,
    neighbor: &'a mut Neighbor,
}

impl<'a> RefNeighbor<'a> {
    pub fn from(interface: &'a mut Interface, ip: Ipv4Addr) -> Option<Self> {
        unsafe {
            let interface = std::ptr::from_mut(interface);
            let neighbor = interface.as_mut().unwrap().neighbors.get_mut(&ip)?;
            Some(Self {
                interface: interface.as_mut().unwrap(),
                neighbor,
            })
        }
    }

    pub fn get_interface(&mut self) -> &mut Interface {
        self.interface
    }

    pub fn get_neighbor(&mut self) -> &mut Neighbor {
        self.neighbor
    }
}
