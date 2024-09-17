mod thread_local_storage;
mod global_storage;
mod config;

pub use global_storage::finalize;

pub fn instant_event(hash: u32, string: &'static str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event_instant(hash, string);
    });
}

pub struct RangeStartGuard {
    repr: RangeStartRepr
}

impl RangeStartGuard {
    pub fn end(self, hash: u32, string: &'static str) {
        thread_local_storage::with_thread_local_tracer(|tracer| {
            tracer.event_range_end(self.repr, hash, string);
        });
    }
}

impl Drop for RangeStartGuard {
    fn drop(&mut self) {
        thread_local_storage::with_thread_local_tracer(|tracer| {
            tracer.event_range_end(self.repr, 0, "");
        });
    }
}

pub fn range_event_start(hash: u32, string: &'static str) -> RangeStartGuard {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        RangeStartGuard {
            repr: tracer.event_range_start(hash, string)
        }
    })
}

pub fn set_cur_thread_name(name: String) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.set_cur_thread_name(name);
    });
}


pub fn flush_thread_local() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.flush(false);
    });
}


pub struct FinalizeGuard;

impl FinalizeGuard {
    pub fn early_drop(self) {}
}

impl Drop for FinalizeGuard {
    fn drop(&mut self) {
        finalize();
    }
}

fn init(_config: SparklesConfigBuilder) {
    // Init global storage
    global_storage::GLOBAL_STORAGE.lock().unwrap().get_or_insert_with(Default::default);
}

pub use config::SparklesConfigBuilder;
use sparkles_core::local_storage::RangeStartRepr;