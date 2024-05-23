use bytes::{Buf, BufMut, Bytes, BytesMut};

pub trait ToBytesMut {
    fn to_bytes_mut(&self) -> BytesMut;
}

pub trait ToBytes {
    fn to_bytes(&self) -> Bytes;
}

impl<T: ToBytesMut> ToBytes for T {
    fn to_bytes(&self) -> Bytes {
        self.to_bytes_mut().freeze()
    }
}

pub trait FromBuf {
    fn from_buf(buf: &mut impl Buf) -> Self;
}

impl<T: ToBytes> ToBytesMut for Vec<T> {
    fn to_bytes_mut(&self) -> BytesMut {
        self.iter().fold(BytesMut::new(), |mut acc, v| {
            acc.extend_from_slice(&v.to_bytes());
            acc
        })
    }
}

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

    pub fn get_buf(&mut self) -> &mut T {
        &mut self.buf
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
            let b = n / 8;
            for i in 1..=b {
                self.buf.put_u8((num >> (8 * (b - i))) as u8);
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

    pub fn extend_from_slice(&mut self, extend: &[u8]) {
        assert_eq!(self.bit, 0);
        self.buf.extend_from_slice(extend);
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

impl ToBytesMut for u8 {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.to_be_bytes());
        buf
    }
}

impl ToBytesMut for u16 {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.to_be_bytes());
        buf
    }
}

impl ToBytesMut for u32 {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.to_be_bytes());
        buf
    }
}

impl ToBytesMut for u64 {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.to_be_bytes());
        buf
    }
}

impl ToBytesMut for u128 {
    fn to_bytes_mut(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&self.to_be_bytes());
        buf
    }
}

impl FromBuf for u8 {
    fn from_buf(buf: &mut impl Buf) -> Self {
        buf.get_u8()
    }
}

impl FromBuf for u16 {
    fn from_buf(buf: &mut impl Buf) -> Self {
        buf.get_u16()
    }
}

impl FromBuf for u32 {
    fn from_buf(buf: &mut impl Buf) -> Self {
        buf.get_u32()
    }
}

impl FromBuf for u64 {
    fn from_buf(buf: &mut impl Buf) -> Self {
        buf.get_u64()
    }
}

impl FromBuf for u128 {
    fn from_buf(buf: &mut impl Buf) -> Self {
        buf.get_u128()
    }
}
