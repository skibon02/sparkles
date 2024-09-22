use std::cell::RefCell;
use std::sync::OnceLock;
use std::thread;
use sparkles_core::config::LocalStorageConfig;
use sparkles_core::headers::{LocalPacketHeader, ThreadInfo};
use sparkles_core::local_storage::{GlobalStorageImpl, LocalStorage};
use crate::global_storage::{GlobalStorage, GLOBAL_STORAGE};

pub struct GlobalStorageRef;
pub type ThreadLocalStorage = LocalStorage<GlobalStorageRef>;

static LOCAL_CONFIG: OnceLock<LocalStorageConfig> = OnceLock::new();

impl GlobalStorageImpl for GlobalStorageRef {
    fn flush(&self, header: &LocalPacketHeader, data: Vec<u8>) {
        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let global_storage_ref = global_storage_ref.get_or_insert_with(|| GlobalStorage::new(Default::default()));
        global_storage_ref.push_buf(header, &data);
    }
}

fn new_local_storage() -> LocalStorage<GlobalStorageRef> {
    let thread_info = thread::current();
    let thread_name = thread_info.name().unwrap_or("Unnamed thread").to_string();
    let thread_id = thread_id::get() as u64;
    let thread_info = ThreadInfo {
        new_thread_name: Some(thread_name.clone()),
        thread_id,
    };
    let config = *LOCAL_CONFIG.get_or_init(LocalStorageConfig::default);
    LocalStorage::new(GlobalStorageRef, Some(thread_info), config)
}

#[inline(always)]
pub fn with_thread_local_tracer<F, R>(f: F) -> R
where F: FnOnce(&mut ThreadLocalStorage) -> R {
    thread_local! {
        static TRACER: RefCell<ThreadLocalStorage> = RefCell::new(new_local_storage());
    }

    TRACER.with_borrow_mut(|tracer| {
        f(tracer)
    })
}