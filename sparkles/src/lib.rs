mod thread_local_storage;
mod global_storage;
mod sender;

pub use global_storage::finalize;
pub use sparkles_core::config::SparklesConfig;

use sparkles_core::local_storage::RangeStartRepr;
use crate::global_storage::GlobalStorage;


/// Use `sparkles-macro::instant_event!("name")` instead
pub fn instant_event(hash: u32, string: &'static str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event_instant(hash, string);
    });
}

/// Created using macro `sparkles-macro::range_event_start!("name")`
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
        tracer.flush(false);
    });
}

/// Guard that will finalize global buffer when dropped
/// Dropping this guard is equivalent to calling `sparkles::finalize`
pub struct FinalizeGuard;

impl FinalizeGuard {
    pub fn early_drop(self) {}
}

impl Drop for FinalizeGuard {
    fn drop(&mut self) {
        finalize();
    }
}

/// Init sparkles with the provided config
///
/// Returns a guard that will finalize global buffer when dropped
pub fn init(config: SparklesConfig) -> FinalizeGuard {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().unwrap().get_or_insert_with(|| GlobalStorage::new(config));

    FinalizeGuard
}

/// Init sparkles with default config
///
/// Returns a guard that will finalize global buffer when dropped
pub fn init_default() -> FinalizeGuard {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().unwrap().get_or_insert_with(|| GlobalStorage::new(Default::default()));

    FinalizeGuard
}