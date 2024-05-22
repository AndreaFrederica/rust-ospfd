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