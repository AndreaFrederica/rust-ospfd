use bytes::{Buf, BufMut, BytesMut};

pub struct Bits<T: Buf> {
    buf: T,
    bit: u8,
    byte: u8,
}

impl<T: Buf> Bits<T> {
    pub fn from(buf: T) -> Self {
        Self {
            buf,
            bit: 0,
            byte: 0,
        }
    }

    pub fn get_un<N: PrimitiveInteger>(&mut self, n: u32) -> N {
        if n % 8 == 0 {
            assert_eq!(self.bit, 0);
            let mut num = 0u128;
            for _ in 0..n / 8 {
                num = (num << 8) | self.buf.get_u8() as u128;
            }
            N::from_u128(num)
        } else {
            let mut num = 0u128;
            for _ in 0..n {
                if self.bit == 0 {
                    self.byte = self.buf.get_u8();
                }
                num = (num << 1) | ((self.byte >> (7 - self.bit)) & 1) as u128;
                self.bit = (self.bit + 1) % 8;
            }
            N::from_u128(num)
        }
    }
}

impl<T: Buf> From<T> for Bits<T> {
    fn from(value: T) -> Self {
        Bits::from(value)
    }
}

pub struct BitsMut {
    buf: BytesMut,
    bit: u8,
    byte: u8,
}

impl BitsMut {
    pub fn new() -> Self {
        Self {
            buf: BytesMut::new(),
            bit: 0,
            byte: 0,
        }
    }

    pub fn put_un<N: PrimitiveInteger>(&mut self, val: N, n: u32) {
        if n % 8 == 0 {
            assert_eq!(self.bit, 0);
            let num = val.to_u128();
            for i in 1..=n / 8 {
                self.buf.put_u8((num >> (8 * (n - i))) as u8);
            }
        } else {
            let num = val.to_u128();
            for i in 1..=n {
                self.byte = (self.byte << 1) | ((num >> (n - i)) & 1) as u8;
                self.bit = (self.bit + 1) % 8;
                if self.bit == 0 {
                    self.buf.put_u8(self.byte);
                    self.byte = 0;
                }
            }
        }
    }
}

impl From<BitsMut> for BytesMut {
    fn from(value: BitsMut) -> Self {
        value.buf
    }
}

pub trait PrimitiveInteger: Copy {
    fn to_u128(self) -> u128;
    fn from_u128(val: u128) -> Self;
}

impl PrimitiveInteger for u8 {
    fn to_u128(self) -> u128 {
        self as u128
    }

    fn from_u128(val: u128) -> Self {
        val as Self
    }
}

impl PrimitiveInteger for u16 {
    fn to_u128(self) -> u128 {
        self as u128
    }

    fn from_u128(val: u128) -> Self {
        val as Self
    }
}

impl PrimitiveInteger for u32 {
    fn to_u128(self) -> u128 {
        self as u128
    }

    fn from_u128(val: u128) -> Self {
        val as Self
    }
}

impl PrimitiveInteger for u64 {
    fn to_u128(self) -> u128 {
        self as u128
    }

    fn from_u128(val: u128) -> Self {
        val as Self
    }
}

impl PrimitiveInteger for u128 {
    fn to_u128(self) -> u128 {
        self as u128
    }

    fn from_u128(val: u128) -> Self {
        val as Self
    }
}
