use std::sync::Arc;

use tokio::task::JoinHandle;

pub type TimerHandle = Option<Arc<JoinHandle<()>>>;
