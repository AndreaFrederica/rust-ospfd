use std::net::Ipv4Addr;

pub const fn ip2hex(ip: Ipv4Addr) -> u32 {
    u32::from_be_bytes(ip.octets())
}

pub const fn hex2ip(hex: u32) -> Ipv4Addr {
    let bytes = hex.to_be_bytes();
    Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
}

#[macro_export]
macro_rules! must {
    ($x:expr $(;do:$op:expr)? $(;ret:$val:expr)? ) => {
        if !($x) {
            $($op;)?
            return $($val)?;
        }
    };
    ($x:expr; dbg: $($arg:tt)*) => {
        if !($x) {
            #[cfg(debug_assertions)]
            crate::log_warning!($($arg)*);
            return;
        }
    };
    ($x:expr; warning: $($arg:tt)*) => {
        if !($x) {
            crate::log_warning!($($arg)*);
            return;
        }
    };
    ($x:expr; error: $($arg:tt)*) => {
        if !($x) {
            crate::log_error!($($arg)*);
            return;
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::constant::AllSPFRouters;

    #[test]
    fn test() {
        assert_eq!(ip2hex(AllSPFRouters), 0xe0000005u32);
        assert_eq!(hex2ip(0xe0000005u32), AllSPFRouters);
    }
}
