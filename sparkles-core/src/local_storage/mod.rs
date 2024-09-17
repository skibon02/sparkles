use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::headers::{LocalPacketHeader, ThreadNameHeader};
use crate::local_storage::id_mapping::IdStore;
use crate::Timestamp;

use crate::timestamp::TimestampProvider;

pub mod id_mapping;

pub const FLUSH_THRESHOLD_PER_THREAD: usize = 10*1024;

pub trait GlobalStorageImpl {
    fn flush(&self, header: LocalPacketHeader, data: Vec<u8>);
    fn put_thread_name(&self, header: ThreadNameHeader);
}

pub struct LocalStorage<G: GlobalStorageImpl> {
    prev_tm: u64,
    accum_tm: u64,

    buf: Vec<u8>,
    id_store: IdStore,

    local_packet_header: LocalPacketHeader,

    thread_name_header: ThreadNameHeader,
    thread_name_changed: bool,

    global_storage_ref: G
}

static CUR_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

impl<G: GlobalStorageImpl> LocalStorage<G> {
    pub fn new(global_storage_ref: G, thread_name: String, thread_id: u64)-> Self {
        let thread_ord_id = CUR_THREAD_ID.fetch_add(1, Ordering::Relaxed) as u64;

        LocalStorage {
            buf: Vec::new(),
            prev_tm: 0,
            accum_tm: 0,

            id_store: Default::default(),
            local_packet_header: LocalPacketHeader {
                thread_ord_id,
                thread_id,
                counts_per_ns: Timestamp::COUNTS_PER_NS,

                ..Default::default()
            },
            thread_name_changed: true,
            thread_name_header: ThreadNameHeader {
                thread_ord_id,
                thread_name,
            },

            global_storage_ref
        }
    }


    #[inline(always)]
    pub fn event(&mut self, hash: u32, string: &str) {
        //      TIMINGS PROVIDED FOR x86-64 PLATFORM ON INTEL i5 12400 CPU

        //      STAGE 1: insert string and get ID. (1.3ns avg)
        let v = self.id_store.insert_and_get_id(hash, string);


        //      STAGE 2: Acquire timestamp and calculate now, dif_tm
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();
        let mut dif_tm = timestamp.wrapping_sub(self.prev_tm);
        self.prev_tm = timestamp;


        //      STAGE 3: Update local info (1ns avg)
        if self.local_packet_header.start_timestamp == 0 {
            // if first event in local packet, init start_timestamp
            self.local_packet_header.start_timestamp = timestamp;
            dif_tm = 0;
        }


        //      STAGE 4: PUSH VALUES (2ns avg)
        let dif_tm_bytes: [u8; 8] = dif_tm.to_le_bytes();
        let dif_tm_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_tm.leading_zeros()) >> 3) as u8; // 0.6ns
        let buf = [v, dif_tm_bytes_len];
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_tm_bytes[..dif_tm_bytes_len as usize]);


        //      STAGE 5: flushing
        if self.buf.len() > FLUSH_THRESHOLD_PER_THREAD {
            self.flush();
        }
    }

    pub fn set_cur_thread_name(&mut self, name: String) {
        self.thread_name_changed = true;
        self.thread_name_header.thread_name = name;
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self) {
        let data = self.buf.clone();
        self.buf.clear();

        if data.len() == 0 {
            // Nothing to flush, ignore
            return;
        }

        self.local_packet_header.end_timestamp = self.prev_tm;
        self.local_packet_header.id_store = self.id_store.clone().into();

        if self.thread_name_changed {
            self.thread_name_changed = false;
            self.global_storage_ref.put_thread_name(self.thread_name_header.clone())
        }
        self.global_storage_ref.flush(self.local_packet_header.clone(), data);

        self.local_packet_header.start_timestamp = 0;
    }

    pub fn finalize(&mut self) {

    }
}

impl<G: GlobalStorageImpl> Drop for LocalStorage<G> {
    fn drop(&mut self) {
        self.flush();
        self.finalize();
    }
}