use std::{io, net::Ipv4Addr};

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
struct routing_item_t {
    pub dest: libc::in_addr_t,
    pub mask: libc::in_addr_t,
    pub nexthop: libc::in_addr_t,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingItem {
    pub dest: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub nexthop: Ipv4Addr,
}

impl From<routing_item_t> for RoutingItem {
    fn from(value: routing_item_t) -> Self {
        Self {
            dest: value.dest.to_be().into(),
            mask: value.mask.to_be().into(),
            nexthop: value.nexthop.to_be().into(),
        }
    }
}

impl From<RoutingItem> for routing_item_t {
    fn from(value: RoutingItem) -> Self {
        Self {
            dest: u32::from(value.dest).to_be(),
            mask: u32::from(value.mask).to_be(),
            nexthop: u32::from(value.nexthop).to_be(),
        }
    }
}

impl std::fmt::Display for RoutingItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}, nexthop: {}",
            self.dest, u32::from(self.mask).count_ones(), self.nexthop
        )
    }
}

mod raw {
    use super::*;
    extern "C" {
        pub fn add_route(r: *const routing_item_t) -> libc::c_int;
        pub fn delete_route(r: *const routing_item_t) -> libc::c_int;
        pub fn get_route_table(arr: *mut routing_item_t, size: libc::c_int) -> libc::c_int;
    }
}

pub fn add_route(r: RoutingItem) -> Result<(), io::Error> {
    if unsafe { raw::add_route(&r.into()) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn delete_route(r: RoutingItem) -> Result<(), io::Error> {
    if unsafe { raw::delete_route(&r.into()) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn get_route_table() -> Result<Vec<RoutingItem>, io::Error> {
    let mut arr = [unsafe { std::mem::zeroed::<routing_item_t>() }; 128];
    let size = unsafe { raw::get_route_table(arr.as_mut_ptr(), arr.len() as libc::c_int) };
    if size < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(arr[..size as usize]
            .iter()
            .map(|it| (*it).into())
            .collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test() {
        let items = get_route_table().unwrap();
        let gateway = items
            .iter()
            .find(|it| it.nexthop != Ipv4Addr::UNSPECIFIED)
            .unwrap();
        let item = RoutingItem {
            dest: Ipv4Addr::new(10, 10, 1, 0),
            mask: Ipv4Addr::new(255, 255, 255, 0),
            nexthop: gateway.nexthop.into(),
        };
        add_route(item).unwrap();
        let items = get_route_table().unwrap();
        assert!(items.contains(&item));
        delete_route(item).unwrap();
        let items = get_route_table().unwrap();
        assert!(!items.contains(&item));
    }
}
