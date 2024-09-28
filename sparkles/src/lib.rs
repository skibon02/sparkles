mod thread_local_storage;
mod global_storage;
pub mod sender;
pub mod config;
mod encoder;

use std::sync::atomic::AtomicBool;
pub use global_storage::finalize;

use sparkles_core::local_storage::RangeStartRepr;
use crate::config::SparklesConfig;
use crate::global_storage::GlobalStorage;

static GLOBAL_FLUSHING_RUNNING: AtomicBool = AtomicBool::new(false);

/// Use `sparkles-macro::instant_event!("name")` instead
pub fn instant_event(hash: u32, string: &'static str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event_instant(hash, string);
    });
}

/// The value is created using macro `sparkles-macro::range_event_start!("name")`
pub struct RangeStartGuard {
    repr: RangeStartRepr,
    ended: bool,
}

impl RangeStartGuard {
    /// Use `sparkles-macro::range_event_end!(guard, "name")` instead
    pub fn end(mut self, hash: u32, string: &'static str) {
        thread_local_storage::with_thread_local_tracer(|tracer| {
            tracer.event_range_end(self.repr, hash, string);
        });
        self.ended = true;
    }
}

impl Drop for RangeStartGuard {
    fn drop(&mut self) {
        if !self.ended {
            thread_local_storage::with_thread_local_tracer(|tracer| {
                tracer.event_range_end(self.repr, 0, "");
            });
        }
    }
}

/// Use `sparkles-macro::range_event_start!("name")` instead
pub fn range_event_start(hash: u32, string: &'static str) -> RangeStartGuard {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        RangeStartGuard {
            repr: tracer.event_range_start(hash, string),
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
/// If you don't need to use it, call `forget()`.
#[must_use]
pub fn init(config: SparklesConfig) -> FinalizeGuard {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().unwrap().get_or_insert_with(|| GlobalStorage::new(config));

    FinalizeGuard
}

/// Init sparkles with default config
///
/// Returns a guard that will finalize global buffer when dropped
///
/// # Attention
/// Do not forget to save finalize guard, returned from this call!
/// If you don't need to use it, call `forget()`.
pub fn init_default() -> FinalizeGuard {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().unwrap().get_or_insert_with(|| GlobalStorage::new(Default::default()));

    FinalizeGuard
}

pub(crate) fn calculate_hash(s: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish() as u32
}