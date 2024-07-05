use std::sync::Mutex;
use std::thread::ThreadId;
use log::info;
use ringbuf::traits::{Consumer, Observer, Producer};
use serde::{Deserialize, Serialize};
use crate::id_mapping::{IdStore, IdStoreMap};

/// Preallocate 50MB for trace buffer
pub const INITIAL_GLOBAL_CAPACITY: usize = 50_000_000;

pub static GLOBAL_STORAGE: Mutex<Option<GlobalStorage>> = Mutex::new(None);

pub struct GlobalStorage {
    inner: ringbuf::LocalRb<ringbuf::storage::Heap<u8>>
}

impl Default for GlobalStorage {
    fn default() -> Self {
        Self {
            inner: ringbuf::LocalRb::new(1_000_000)
        }
    }

}

impl GlobalStorage {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push_buf(&mut self, header: LocalPacketHeader, buf: &[u8]) {
        info!("Flushing local packet: {:?}", header);
        let header = bincode::serialize(&header).unwrap();
        let header_len = header.len().to_be_bytes();
        info!("Local packet len: {}", header.len());

        self.inner.push_slice(&header_len);
        self.inner.push_slice(&header);
        self.inner.push_slice(&buf);

        if self.inner.occupied_len() > 100_000_000 {
            self.inner.clear();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LocalPacketHeader {
    pub thread_name: String,
    pub thread_id: u64,

    pub initial_timestamp: u64,
    pub end_timestamp: u64,

    pub id_store: IdStoreMap,
    pub buf_length: usize,
}