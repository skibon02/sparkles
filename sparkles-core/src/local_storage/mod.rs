use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::headers::{LocalPacketHeader, ThreadNameHeader};
use crate::local_storage::id_mapping::{EventType, IdStoreRepr};
use crate::Timestamp;

use crate::timestamp::TimestampProvider;

pub mod id_mapping;

/// Todo: Make this configurable
pub const FLUSH_THRESHOLD_PER_THREAD: usize = 10*1024;

pub trait GlobalStorageImpl {
    fn flush(&self, header: LocalPacketHeader, data: Vec<u8>);
    fn put_thread_name(&self, header: ThreadNameHeader);
}

pub struct LocalStorage<G: GlobalStorageImpl> {
    prev_tm: u64,
    accum_tm: u64,

    buf: Vec<u8>,
    id_store: IdStoreRepr,

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
    pub fn event_range_start(&self, hash: u32, name: &'static str) -> RangeStartRepr {
        RangeStartRepr {
            hash,
            name,
            thread_ord_id: self.local_packet_header.thread_ord_id,
            tm: Timestamp::now(),
        }
    }

    #[inline(always)]
    pub fn event_range_end(&mut self, range_start: RangeStartRepr, hash: u32, name: &'static str) {
        // 1) Insert event start string
        let start_id = self.id_store.insert_and_get_id(range_start.hash, range_start.name, EventType::RangeStart);
        if hash != 0 {
            let end_id = self.id_store.insert_and_get_id(hash, name, EventType::RangeEnd(start_id));
            self.range_event(start_id, end_id, range_start.tm);
        }
        else {
            // 0 == no end event name
            self.range_event(start_id, 0, range_start.tm);
        }
    }

    #[inline(always)]
    fn range_event(&mut self, start_id: u8, end_id: u8, start_tm: u64) {
        //      STAGE 2: Acquire timestamp and calculate now, dif_tm
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();

        //      STAGE 3: Update local info
        let dif_tm = self.update_local_info(timestamp);
        let dur = timestamp.wrapping_sub(start_tm);
        let dur_bytes: [u8; 8] = dur.to_le_bytes();
        let dur_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dur.leading_zeros()) >> 3) as u8;

        //      STAGE 4: PUSH VALUES
        let dif_tm_bytes: [u8; 8] = dif_tm.to_le_bytes();
        let dif_tm_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_tm.leading_zeros()) >> 3) as u8;
        let buf = [start_id, dif_tm_bytes_len | 0x80, end_id, dur_len]; // add flag to indicate range event
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_tm_bytes[..dif_tm_bytes_len as usize]);
        self.buf.extend_from_slice(&dur_bytes[..dur_len as usize]);

        //      STAGE 5: flushing
        self.auto_flush();
    }


    #[inline(always)]
    pub fn event_instant(&mut self, hash: u32, string: &'static str) {
        //      STAGE 1: insert string and get ID.
        let id = self.id_store.insert_and_get_id(hash, string, EventType::Instant);
        self.event(id);
    }

    #[inline(always)]
    fn event(&mut self, id: u8) {
        //      STAGE 2: Acquire timestamp and calculate now, dif_tm
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();

        //      STAGE 3: Update local info
        let dif_tm = self.update_local_info(timestamp);

        //      STAGE 4: PUSH VALUES
        let dif_tm_bytes: [u8; 8] = dif_tm.to_le_bytes();
        let dif_tm_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_tm.leading_zeros()) >> 3) as u8;
        let buf = [id, dif_tm_bytes_len];
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_tm_bytes[..dif_tm_bytes_len as usize]);


        //      STAGE 5: flushing
        self.auto_flush();
    }

    #[inline(always)]
    fn update_local_info(&mut self, timestamp: u64) -> u64 {
        let mut dif_tm = timestamp.wrapping_sub(self.prev_tm);
        self.prev_tm = timestamp;
        if self.local_packet_header.start_timestamp == 0 {
            self.local_packet_header.start_timestamp = timestamp;
            dif_tm = 0;
        }
        dif_tm
    }

    pub fn set_cur_thread_name(&mut self, name: String) {
        self.thread_name_changed = true;
        self.thread_name_header.thread_name = name;
    }

    /// Check buffer length, and flush if the buffer is full
    #[inline(always)]
    pub fn auto_flush(&mut self) {
        if self.buf.len() >= FLUSH_THRESHOLD_PER_THREAD {
            self.flush(false);
        }
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self, _finalize: bool) {
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
}

impl<G: GlobalStorageImpl> Drop for LocalStorage<G> {
    fn drop(&mut self) {
        self.flush(true);
    }
}

#[derive(Copy, Clone)]
pub struct RangeStartRepr {
    hash: u32,
    name: &'static str,
    thread_ord_id: u64,
    tm: u64,
}