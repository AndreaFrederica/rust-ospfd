use std::sync::Arc;

use tokio::sync::Mutex;

use crate::log_error;

pub trait Runnable {
    fn run(&mut self);
}

pub trait AsyncRunnable {
    fn run_async(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

pub trait Daemon {
    async fn run_forever(self);
}

impl<T: Runnable + Send> AsyncRunnable for T {
    async fn run_async(&mut self) {
        tokio::task::block_in_place(|| self.run());
    }
}

impl<T: AsyncRunnable + Send + 'static> Daemon for T {
    async fn run_forever(self) {
        let daemon = Arc::new(Mutex::new(self));
        loop {
            let daemon = daemon.clone();
            let hd = tokio::task::spawn(async move {
                let mut daemon = daemon.lock().await;
                daemon.run_async().await;
            });
            if hd.await.is_err() {
                log_error!(
                    "An error occurred while running {}",
                    std::any::type_name::<T>()
                );
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        }
    }
}
