#![feature(effects)]
#![feature(thread_id_value)]
#![feature(option_get_or_insert_default)]

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

mod thread_local_storage;
mod timestamp;
mod id_mapping;
mod global_storage;
pub use global_storage::LocalPacketHeader;
pub use global_storage::finalize;

pub fn event(hash: u32, string: &str) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(hash, string);
    });
}

static PACKET_NUM: AtomicUsize = AtomicUsize::new(0);

pub fn flush_thread_local() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.flush();
        PACKET_NUM.fetch_add(1, Ordering::Relaxed);
    });
}