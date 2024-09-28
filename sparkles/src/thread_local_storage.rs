use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::thread;
use sparkles_core::config::LocalStorageConfig;
use sparkles_core::headers::{LocalPacketHeader, ThreadInfo};
use sparkles_core::local_storage::{GlobalStorageImpl, LocalStorage};
use crate::GLOBAL_FLUSHING_RUNNING;
use crate::global_storage::{GlobalStorage, GLOBAL_STORAGE};

pub struct GlobalStorageRef;
pub type ThreadLocalStorage = LocalStorage<GlobalStorageRef>;

static LOCAL_CONFIG: OnceLock<LocalStorageConfig> = OnceLock::new();
pub(crate) fn set_local_storage_config(config: LocalStorageConfig) {
    let _ = LOCAL_CONFIG.set(config);
}

impl GlobalStorageImpl for GlobalStorageRef {
    fn flush(&self, header: &LocalPacketHeader, data: &[u8]) {
        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let global_storage_ref = global_storage_ref.get_or_insert_with(|| GlobalStorage::new(Default::default()));
        global_storage_ref.push_buf(header, data);
    }
    fn try_flush(&self, header: &LocalPacketHeader, data: &[u8]) -> bool {
        if let Ok(mut global_storage_ref) = GLOBAL_STORAGE.try_lock() {
            let global_storage_ref = global_storage_ref.get_or_insert_with(|| GlobalStorage::new(Default::default()));
            global_storage_ref.push_buf(header, data);
            true
        }
        else {
            false
        }
    }
    fn is_buf_available(&self) -> bool {
        !GLOBAL_FLUSHING_RUNNING.load(Ordering::Relaxed)
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