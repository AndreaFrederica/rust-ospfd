use std::sync::Arc;

use tokio::sync::RwLock;

pub struct Router {
    pub router_id: u32,
    pub router_type: RType,
    pub router_state: RState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RType {
    DR,
    BDR,
    Other,
}

pub enum RState {
    None,
    //todo!
}

impl Router {
    pub fn new(router_id: u32) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            router_id,
            router_type: RType::Other,
            router_state: RState::None,
        }))
    }
}
