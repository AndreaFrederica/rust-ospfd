use tokio::task::JoinHandle;

pub type TimerHandle = Option<JoinHandle<()>>;
