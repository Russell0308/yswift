use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use yrs::Origin;

/// A live observer. yrs 0.28 registers observers under a key instead of
/// returning a handle, so this owns the key and unobserves when Swift drops it.
pub(crate) struct YSubscription {
    cancel: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl YSubscription {
    pub(crate) fn new(cancel: impl FnOnce() + Send + 'static) -> YSubscription {
        YSubscription { cancel: Mutex::new(Some(Box::new(cancel))) }
    }
}

impl Drop for YSubscription {
    fn drop(&mut self) {
        let cancel = self.cancel.get_mut().ok().and_then(|c| c.take());
        if let Some(cancel) = cancel {
            // Unobserving can fail (e.g. a transaction is still open); never panic in drop.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cancel));
        }
    }
}

/// A unique observer key per subscription.
pub(crate) fn next_key() -> Origin {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    Origin::from(format!("yniffi-subscription-{}", NEXT.fetch_add(1, Ordering::Relaxed)).as_str())
}

/// Moves a yrs handle (raw branch pointer inside) into a Send closure.
pub(crate) struct SendBox<T>(pub(crate) T);
unsafe impl<T> Send for SendBox<T> {}
unsafe impl<T> Sync for SendBox<T> {}
