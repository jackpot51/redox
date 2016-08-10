pub use alloc::arc::{Arc, Weak};
pub use core::sync::atomic;
pub use self::mutex::{Mutex, MutexGuard, StaticMutex};
pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use self::once::Once;

pub mod mpsc;
mod futex;
mod mutex;
mod once;
mod rwlock;
