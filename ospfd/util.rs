use std::net::Ipv4Addr;

#[macro_export]
macro_rules! ip {
    ($x:expr) => {
        std::net::IpAddr::V4($x)
    };
}

#[macro_export]
macro_rules! hex {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        ($a as u32) << 24 | ($b as u32) << 16 | ($c as u32) << 8 | ($d as u32)
    };
}

pub const fn ip2hex(ip: Ipv4Addr) -> u32 {
    u32::from_be_bytes(ip.octets())
}

pub const fn hex2ip(hex: u32) -> Ipv4Addr {
    let bytes = hex.to_be_bytes();
    Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::constant::AllSPFRouters;

    #[test]
    fn test() {
        assert_eq!(ip2hex(AllSPFRouters), 0xf4000005);
        assert_eq!(hex2ip(0xf4000005), AllSPFRouters);
    }
}
