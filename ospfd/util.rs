use std::net::Ipv4Addr;

pub const fn ip2hex(ip: Ipv4Addr) -> u32 {
    u32::from_be_bytes(ip.octets())
}

pub const fn hex2ip(hex: u32) -> Ipv4Addr {
    let bytes = hex.to_be_bytes();
    Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
}

#[derive(Debug, Default)]
pub struct AbortHandle(Option<tokio::task::AbortHandle>);

impl AbortHandle {
    pub fn abort(&self) {
        if let Some(ref handle) = self.0 {
            handle.abort();
        }
    }

    pub fn is_finished(&self) -> bool {
        if let Some(ref handle) = self.0 {
            handle.is_finished()
        } else {
            true
        }
    }
}

impl Drop for AbortHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

impl TryFrom<AbortHandle> for tokio::task::AbortHandle {
    type Error = &'static str;
    fn try_from(mut handle: AbortHandle) -> Result<Self, Self::Error> {
        if let Some(handle) = handle.0.take() {
            Ok(handle)
        } else {
            Err("AbortHandle is empty")
        }
    }
}

impl From<tokio::task::AbortHandle> for AbortHandle {
    fn from(handle: tokio::task::AbortHandle) -> Self {
        Self(Some(handle))
    }
}

impl<T> From<&tokio::task::JoinHandle<T>> for AbortHandle {
    fn from(handle: &tokio::task::JoinHandle<T>) -> Self {
        Self(Some(handle.abort_handle()))
    }
}

impl<T> From<tokio::task::JoinHandle<T>> for AbortHandle {
    fn from(handle: tokio::task::JoinHandle<T>) -> Self {
        Self(Some(handle.abort_handle()))
    }
}

#[macro_export]
macro_rules! must {
    ($x:expr $(;else:$op:expr)? $(;ret:$val:expr)? $(;)?) => {
        if !($x) {
            $($op;)?
            return $($val)?;
        }
    };
    ($x:expr $(;else:$op:expr)?; continue $(;)?) => {
        if !($x) {
            $($op;)?
            continue;
        }
    };
    ($x:expr $(;else:$op:expr)?; break $($val:expr)? $(;)?) => {
        if !($x) {
            $($op;)?
            break $($val)?;
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

#[macro_export]
macro_rules! guard {
    ($x:pat = $y:expr $(;else:$op:expr)? $(;ret:$val:expr)? $(;)?) => {
        let $x = $y else {
            $($op;)?
            return $($val)?;
        };
    };
    ($x:pat = $y:expr $(;else:$op:expr)?; continue $(;)?) => {
        let $x = $y else {
            $($op;)?
            continue;
        };
    };
    ($x:pat = $y:expr $(;else:$op:expr)?; break $($val:expr)? $(;)?) => {
        let $x = $y else {
            $($op;)?
            break $($val)?;
        };
    };
    ($x:pat = $y:expr; dbg: $($arg:tt)*) => {
        let $x = $y else {
            #[cfg(debug_assertions)]
            crate::log_warning!($($arg)*);
            return;
        };
    };
    ($x:pat = $y:expr; warning: $($arg:tt)*) => {
        let $x = $y else {
            crate::log_warning!($($arg)*);
            return;
        };
    };
    ($x:pat = $y:expr; error: $($arg:tt)*) => {
        let $x = $y else {
            crate::log_error!($($arg)*);
            return;
        };
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
