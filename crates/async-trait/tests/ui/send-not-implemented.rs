use async_trait::async_trait;
use parking_lot::Mutex;

async fn f() {}

#[async_trait]
trait Test {
    async fn test(&self) {
        let Mutex = Mutex::new(());
        let _guard = Mutex.lock();
        f().await;
    }

    async fn test_ret(&self) -> bool {
        let Mutex = Mutex::new(());
        let _guard = Mutex.lock();
        f().await;
        true
    }
}

fn main() {}
