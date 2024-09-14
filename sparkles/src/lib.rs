use std::sync::atomic::{AtomicUsize, Ordering};

mod thread_local_storage;
mod id_mapping;
mod global_storage;
mod config;

pub use global_storage::LocalPacketHeader;
pub use global_storage::finalize;

pub fn event(hash: u32, string: &str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(hash, string);
    });
}


pub fn flush_thread_local() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.flush();
    });
}

static MAIN_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

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

    // Save main thread id to properly finalize on exit
    let main_thread_id = thread_id::get();
    MAIN_THREAD_ID.store(main_thread_id, Ordering::Relaxed);
}

pub use config::SparklesConfigBuilder;