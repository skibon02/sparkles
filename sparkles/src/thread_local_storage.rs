use std::cell::RefCell;
use std::thread;
use sparkles_core::headers::{LocalPacketHeader, ThreadNameHeader};
use sparkles_core::local_storage::{GlobalStorageImpl, LocalStorage};
use crate::global_storage::GLOBAL_STORAGE;

pub struct GlobalStorageRef;
pub type ThreadLocalStorage = LocalStorage<GlobalStorageRef>;

impl GlobalStorageImpl for GlobalStorageRef {
    fn flush(&self, header: LocalPacketHeader, data: Vec<u8>) {
        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let global_storage_ref = global_storage_ref.get_or_insert_with(Default::default);
        global_storage_ref.push_buf(header, &data);
    }
    fn put_thread_name(&self, header: ThreadNameHeader) {
        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let global_storage_ref = global_storage_ref.get_or_insert_with(Default::default);
        global_storage_ref.update_thread_name(header);
    }
}

fn new_local_storage() -> LocalStorage<GlobalStorageRef> {
    let thread_info = thread::current();
    let thread_name = thread_info.name().unwrap_or("Unnamed thread").to_string();
    let thread_id = thread_id::get() as u64;
    LocalStorage::new(GlobalStorageRef, thread_name, thread_id)
}

#[inline(always)]
pub fn with_thread_local_tracer<F>(f: F)
where F: FnOnce(&mut ThreadLocalStorage) {
    thread_local! {
        static TRACER: RefCell<ThreadLocalStorage> = RefCell::new(new_local_storage());
    }

    TRACER.with_borrow_mut(|tracer| {
        f(tracer)
    });
}