#![ cfg_attr( feature = "unstable-thread-id", feature(thread_id_value) ) ]

mod thread_local_storage;
mod global_storage;
pub mod sender;
pub mod config;
pub mod external_events;
pub mod monotonic;

pub use sparkles_macro::*;
pub mod core {
    pub use sparkles_core::*;
}

use std::sync::atomic::{AtomicBool, AtomicUsize};
use parking_lot::{Condvar, Mutex};
use log::{info, warn};
pub use global_storage::finalize;

use sparkles_core::local_storage::RangeStartRepr;
use sparkles_core::StaticNameRepr;
use crate::config::SparklesConfig;
use crate::global_storage::GlobalStorage;

static GLOBAL_FLUSHING_RUNNING: AtomicBool = AtomicBool::new(false);
static THREAD_LOCAL_NOTIFICATION: AtomicUsize = AtomicUsize::new(0);

/// Capture timestamp of the current point in time and create an event with the given name.
///
/// Correct usage:
/// 1) `sparkles::instant_event!("event name")`
/// 2) `sparkles::instant_event(sparkles::static_name!("event
/// name))`
///
/// Both variants are equivalent
pub fn instant_event(name: StaticNameRepr) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event_instant(name);
    });
}

/// The value is created using macro `sparkles::range_event_start!("event name")`
pub struct RangeStartGuard {
    repr: RangeStartRepr,
    ended: bool,
}

impl RangeStartGuard {
    /// If you want to end the range event, simply drop it: `drop(g);`
    ///
    /// However, if you want to end the range with a custom name (for example representing operation end reason), you can use this method.
    /// Correct usage:
    /// 1) `sparkles::range_event_end!(guard, "range end name")`
    /// 2) `guard.end(sparkles::static_name!("range end name"))`
    ///
    /// Both variants are equivalent
    pub fn end(mut self, name: StaticNameRepr) {
        thread_local_storage::with_thread_local_tracer(|tracer| {
            tracer.event_range_end(self.repr, name);
        });
        self.ended = true;
    }
}

impl Drop for RangeStartGuard {
    fn drop(&mut self) {
        if !self.ended {
            thread_local_storage::with_thread_local_tracer(|tracer| {
                tracer.event_range_end(self.repr, StaticNameRepr::empty());
            });
        }
    }
}

/// Begin a range event with the given name at current point in time.
///
/// Correct usage:
/// 1) `sparkles::range_event_start!("range start name")`
/// 2) `sparkles::range_event_start(sparkles::static_name!("range start name"))`
#[must_use]
pub fn range_event_start(name: StaticNameRepr) -> RangeStartGuard {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        RangeStartGuard {
            repr: tracer.event_range_start(name),
            ended: false,
        }
    })
}

/// Update current visible thread name. It will override the previous name when parsed
pub fn set_cur_thread_name(name: String) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.set_cur_thread_name(name);
    });
}

/// Manually flush all events from thread-local buffer to the global buffer
pub fn flush_thread_local() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.flush(true);
    });
}

/// Guard that will finalize global buffer when dropped
/// Dropping this guard is equivalent to calling `sparkles::finalize`
pub struct FinalizeGuard;

impl FinalizeGuard {
    pub fn early_drop(self) {}
    pub fn forget(self) {
        std::mem::forget(self)
    }
}

impl Drop for FinalizeGuard {
    fn drop(&mut self) {
        finalize();
    }
}

/// Init sparkles with the provided config
///
/// Returns a guard that will finalize global buffer when dropped
///
/// # Attention
/// Do not forget to save finalize guard, returned from this call!
/// If you drop it too early, sparkles thread will be finished and no more data will be recorded!
#[must_use]
pub fn init(config: SparklesConfig) -> FinalizeGuard {
    // Init global storage
    let mut storage_lock = global_storage::GLOBAL_STORAGE.lock();
    if storage_lock.is_some() {
        warn!("Global storage was initialized before! Ignoring new config");
    }
    storage_lock.get_or_insert_with(|| GlobalStorage::new(config));

    FinalizeGuard
}

/// Init sparkles with default config
///
/// Returns a guard that will finalize global buffer when dropped
///
/// # Attention
/// Do not forget to save finalize guard, returned from this call!
/// If you drop it too early, sparkles thread will be finished and no more data will be recorded!
pub fn init_default() -> FinalizeGuard {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().get_or_insert_with(|| GlobalStorage::new(Default::default()));

    FinalizeGuard
}

pub(crate) fn calculate_hash(s: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish() as u32
}


static SOMEONE_CONNECTED: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
pub(crate) fn on_client_connect() {
    let (lock, cvar) = &SOMEONE_CONNECTED;
    let mut connected = lock.lock();
    *connected = true;
    cvar.notify_all();
}

/// If you use UDP, you can wait until someone connects to the server to start sending data
/// 
/// This function will block until someone connects without timeouts!
/// 
/// If UDP is not used, this function will return immediately
pub fn wait_client_connected() {
    use crate as sparkles;
    #[cfg(feature="self-tracing")]
    let g = sparkles_macro::range_event_start!("[internal] Waiting for client connect");
    let (lock, cvar) = &SOMEONE_CONNECTED;
    let mut connected = lock.lock();
    if !*connected {
        info!("Waiting for client connection...");
    }
    while !*connected {
        cvar.wait(&mut connected);
    }
}