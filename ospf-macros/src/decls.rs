#[macro_export]
macro_rules! raw_hex {
    ($raw:literal) => {
        {
            let mut vec = Vec::new();
            let mut iter = $raw.chars();
            while let Some(c) = iter.next() {
                let byte = match iter.next() {
                    Some(c2) => u8::from_str_radix(&format!("{}{}", c, c2), 16).unwrap(),
                    None => panic!("Invalid raw hex string"),
                };
                vec.push(byte);
            }
            vec
        }
    };
}

#[macro_export]
macro_rules! ipv4 {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        IpAddr::V4(Ipv4Addr::new($a, $b, $c, $d))
    };
}

#[macro_export]
macro_rules! ip2hex {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        ($a as u32) << 24 | ($b as u32) << 16 | ($c as u32) << 8 | ($d as u32)
    };
}
