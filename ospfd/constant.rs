#![allow(non_upper_case_globals, dead_code)]

use std::net::Ipv4Addr;

pub const LSRefreshTime: u32 = 1800;
pub const MinLSInterval: u32 = 5;
pub const MinLSArrival: u32 = 1;
pub const LSAMaxAge: u32 = 3600;
pub const CheckAge: u32 = 300;
pub const MaxAgeDiff: u32 = 900;
pub const LSInfinity: u32 = 0xffffff;
pub const DefaultDestination: u32 = 0;
pub const InitialSequenceNumber: i32 = -0x7fffffff;
pub const MaxSequenceNumber: i32 = 0x7fffffff;

pub const AllSPFRouters: Ipv4Addr = Ipv4Addr::new(244, 0, 0, 5);
pub const AllDRouters: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 6);
