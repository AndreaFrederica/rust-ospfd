#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceState {
    Down,
    Loopback,
    Waiting,
    PointToPoint,
    DROther,
    Backup,
    DR,
}
