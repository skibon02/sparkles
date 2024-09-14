use std::cell::RefCell;
use std::thread;
use log::debug;
use crate::global_storage::{GLOBAL_STORAGE, LocalPacketHeader};
use crate::id_mapping::IdStore;

use sparkles_core::Timestamp;
use sparkles_core::TimestampProvider;

pub const FLUSH_THRESHOLD_PER_THREAD: usize = 10*1024;

pub struct ThreadLocalStorage {
    start_timestamp: u64,
    accum_pr: u64,
    last_now: u16,
    buf: Vec<u8>,
    id_store: IdStore,

    prev_pr: u64,
    thread_id: usize,
    //todo: name can change
    thread_name: String,
}

impl ThreadLocalStorage {
    pub fn new()-> Self {
        let thread_info = thread::current();
        let thread_name = thread_info.name().unwrap_or("Unnamed thread");

        ThreadLocalStorage {
            buf: Vec::with_capacity(FLUSH_THRESHOLD_PER_THREAD + 10),
            id_store: IdStore::new(),
            start_timestamp: 0,
            accum_pr: 0,
            last_now: 0,
            prev_pr: 0,
            thread_id: thread_id::get(),
            thread_name: thread_name.to_string()
        }
    }


    #[inline(always)]
    pub fn event(&mut self, hash: u32, string: &str) {
        //      TIMINGS PROVIDED FOR x86-64 PLATFORM ON INTEL i5 12400 CPU

        //      STAGE 1: insert string and get ID. (1.3ns avg)
        let v = self.id_store.insert_and_get_id(hash, string);


        //      STAGE 2: Acquire timestamp and calculate now, dif_pr
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();
        let now_pr = timestamp >> 16;
        let now = timestamp as u16;
        let mut dif_pr = now_pr.wrapping_sub(self.prev_pr);


        //      STAGE 3: Update local info (1ns avg)
        self.prev_pr = now_pr;
        if self.start_timestamp == 0 {
            // if first event in local packet, init start_timestamp
            self.start_timestamp = timestamp;
            dif_pr = 0;
        }
        else {
            self.accum_pr += dif_pr;
        }
        self.last_now = now;


        //      STAGE 4: PUSH VALUES (2ns avg)
        let dif_pr_bytes: [u8; 8] = dif_pr.to_le_bytes();
        let dif_pr_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_pr.leading_zeros()) >> 3) as u8; // 0.6ns
        let buf = [v, dif_pr_bytes_len, now as u8, (now >> 8) as u8];
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_pr_bytes[..dif_pr_bytes_len as usize]);


        //      STAGE 5: flushing
        if self.buf.len() > FLUSH_THRESHOLD_PER_THREAD {
            self.flush();
        }
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self) {
        let data = self.buf.clone();
        self.buf.clear();
        
        if data.len() == 0 {
            // Nothing to flush, ignore
            return;
        }

        let header = LocalPacketHeader {
            thread_name: self.thread_name.clone(),
            thread_id: self.thread_id as u64,
            initial_timestamp: self.start_timestamp,
            end_timestamp: ((self.start_timestamp & 0xFFFF_FFFF_FFFF_0000) + (self.accum_pr << 16)) | self.last_now as u64,
            buf_length: data.len() as u64,
            id_store: self.id_store.clone().into(),
            counts_per_ns: Timestamp::COUNTS_PER_NS
        };

        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let global_storage_ref = global_storage_ref.get_or_insert_with(Default::default);
        global_storage_ref.push_buf(header, &*data);

        self.start_timestamp = 0;
        self.accum_pr = 0;
    }
}

impl Drop for ThreadLocalStorage {
    fn drop(&mut self) {
        self.flush();

        let thread = thread::current();
        let id = thread_id::get();
        debug!("Dropping TLS from thread {:?}", thread.name());
        // if id == MAIN_THREAD_ID.load(Ordering::Relaxed) {
        //     debug!("Main drop detected!");
        // }
    }
}

#[inline(always)]
pub fn with_thread_local_tracer<F>(f: F)
where F: FnOnce(&mut ThreadLocalStorage) {
    thread_local! {
        static TRACER: RefCell<ThreadLocalStorage> = RefCell::new(ThreadLocalStorage::new());
    }

    TRACER.with_borrow_mut(|tracer| {
        f(tracer)
    });
}