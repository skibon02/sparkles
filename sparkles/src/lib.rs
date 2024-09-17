mod thread_local_storage;
mod global_storage;
mod config;

pub use global_storage::finalize;

pub fn event(hash: u32, string: &str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(hash, string);
    });
}

pub fn set_cur_thread_name(name: String) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.set_cur_thread_name(name);
    });
}


pub fn flush_thread_local() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.flush();
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