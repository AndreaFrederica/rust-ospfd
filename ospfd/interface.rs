use std::sync::Arc;

use crate::router::Router;
use tokio::sync::RwLock;

pub struct Interface {
    pub router: Arc<RwLock<Router>>,
    pub area_id: u32,
}

impl Interface {
    pub fn new(router: Arc<RwLock<Router>>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self { router, area_id: 0 }))
    }
}
