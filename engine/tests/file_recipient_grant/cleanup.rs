use super::fixture::Fixture;
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin};

/// Catch assertion panics while the fixture still exists, release all futures/transactions,
/// then reclaim the exact disposable namespace before preserving the original failure.
pub async fn checked<T, F>(mut f: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let result = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    match result {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
