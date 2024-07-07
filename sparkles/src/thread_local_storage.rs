use std::cell::RefCell;
use log::info;
use crate::global_storage::{GLOBAL_STORAGE, LocalPacketHeader};
use crate::id_mapping::IdStore;

pub const FLUSH_THRESHOLD: usize = 10_000;

pub struct ThreadLocalStorage {
    start_timestamp: u64,
    accum_pr: u64,
    last_now: u16,
    buf: Vec<u8>,
    id_store: IdStore,

    prev_pr: u64,
}

impl ThreadLocalStorage {
    pub const fn new()-> Self {
        ThreadLocalStorage {
            buf: Vec::new(),
            id_store: IdStore::new(),
            start_timestamp: 0,
            accum_pr: 0,
            last_now: 0,
            prev_pr: 0,
        }
    }


    pub fn event(&mut self, hash: u32, string: &str) {
        let timestamp = crate::timestamp::now();
        // let now = 8234721;
        let now_pr = (timestamp >> 16) as u64;
        let now = timestamp as u16;
        let mut dif_pr = now_pr.wrapping_sub(self.prev_pr) & 0xFFFF_FFFF_FFFF;
        self.prev_pr = now_pr;
        let mut buf = [0; 11];
        let v = self.id_store.insert_and_get_id(hash, string);

        if self.start_timestamp == 0 {
            // if first event in local packet, init start_timestamp
            self.start_timestamp = timestamp;
            // ignore dif_pr as we have start_timestamp
            dif_pr = 0;
        }
        else {
            self.accum_pr += dif_pr;
        }
        self.last_now = now;

        buf[0] = v | 0x80;
        buf[1] = (now >> 8) as u8;
        buf[2] = now as u8;

        let mut ind = 3;
        // While value is 64-16 = 48 bits, we send 7 bits at a time
        while dif_pr > 0 {
            buf[ind] = dif_pr as u8 & 0x7F;

            dif_pr >>= 7;
            ind += 1;
        }

        // Write event packet
        self.buf.extend_from_slice(&buf[..ind]);

        // Automatic flush
        if self.buf.len() > FLUSH_THRESHOLD {
            self.flush();
        }
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self) {
        let data = self.buf.clone().into_boxed_slice();
        self.buf.clear();
        let mut global_storage_ref = GLOBAL_STORAGE.lock().unwrap();
        let mut global_storage_ref = global_storage_ref.get_or_insert_default();

        let thread_info = std::thread::current();
        let header = LocalPacketHeader {
            thread_name: thread_info.name().unwrap_or("unnamed").to_string(),
            thread_id: thread_info.id().as_u64().get(),
            initial_timestamp: self.start_timestamp,
            end_timestamp: ((self.start_timestamp & 0xFFFF_FFFF_FFFF_0000) + (self.accum_pr << 16)) | self.last_now as u64,
            buf_length: data.len(),
            id_store: self.id_store.clone().into()
        };
        global_storage_ref.push_buf(header, &*data);

        self.start_timestamp = 0;
        self.accum_pr = 0;
    }
}


pub fn with_thread_local_tracer<F>(f: F)
where F: FnOnce(&mut ThreadLocalStorage) {
    thread_local! {
        static TRACER: RefCell<ThreadLocalStorage> = RefCell::new(ThreadLocalStorage::new());
    }

    TRACER.with_borrow_mut(|tracer| {
        f(tracer)
    });
}