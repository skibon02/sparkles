use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::config::LocalStorageConfig;
use crate::headers::{LocalPacketHeader, ThreadInfo};
use crate::local_storage::id_mapping::{EventType, IdStoreRepr};
use crate::Timestamp;

use crate::timestamp::TimestampProvider;

pub mod id_mapping;

pub trait GlobalStorageImpl {
    fn flush(&self, header: &LocalPacketHeader, data: Vec<u8>);
}

pub struct LocalStorage<G: GlobalStorageImpl> {
    config: LocalStorageConfig,
    
    prev_tm: u64,

    buf: Vec<u8>,
    id_store: IdStoreRepr,

    local_packet_header: LocalPacketHeader,

    global_storage_ref: G,
    last_range_ord_id: u8
}

static CUR_THREAD_ID: AtomicUsize = AtomicUsize::new(0);

impl<G: GlobalStorageImpl> LocalStorage<G> {
    pub fn new(global_storage_ref: G, thread_info: Option<ThreadInfo>, config: LocalStorageConfig)-> Self {
        let thread_ord_id = CUR_THREAD_ID.fetch_add(1, Ordering::Relaxed) as u64;

        LocalStorage {
            config,
            buf: Vec::new(),
            prev_tm: 0,

            id_store: Default::default(),
            local_packet_header: LocalPacketHeader {
                thread_ord_id,
                thread_info,

                ..Default::default()
            },

            global_storage_ref,
            last_range_ord_id: 0
        }
    }

    fn new_range_ord_id(&mut self) -> u8 {
        let range_ord_id = self.last_range_ord_id;
        self.last_range_ord_id = self.last_range_ord_id.wrapping_add(1);
        range_ord_id
    }

    #[inline(always)]
    pub fn event_range_start(&mut self, hash: u32, name: &str) -> RangeStartRepr {
        // On a new range event we acquire new range_ord_id to match start and end events
        let range_ord_id = self.new_range_ord_id();
        let start_id = self.id_store.insert_and_get_id(hash, name, EventType::RangeStart);
        self.range_event(Some(start_id), range_ord_id);

        RangeStartRepr {
            range_ord_id,
            range_start_id: start_id,

            _not_send: PhantomData
        }
    }

    #[inline(always)]
    pub fn event_range_end(&mut self, range_start: RangeStartRepr, hash: u32, name: &str) {
        let range_ord_id = range_start.range_ord_id;
        let start_id = range_start.range_start_id;
        if hash != 0 {
            let end_id = self.id_store.insert_and_get_id(hash, name, EventType::RangeEnd(start_id));
            self.range_event(Some(end_id), range_ord_id);
        }
        else {
            self.range_event(None, range_ord_id);
        }
    }

    #[inline(always)]
    fn range_event(&mut self, id: Option<u8>, range_ord_id: u8) {
        //      STAGE 2: Acquire timestamp and calculate now, dif_tm
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();

        //      STAGE 3: Update local info
        let dif_tm = self.update_local_info(timestamp);

        //      STAGE 4: PUSH VALUES
        let dif_tm_bytes: [u8; 8] = dif_tm.to_le_bytes();
        let dif_tm_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_tm.leading_zeros()) >> 3) as u8;
        let buf = match id {
            Some(id) => [id, dif_tm_bytes_len | 0x80, range_ord_id],
            None => [0, dif_tm_bytes_len | 0xC0, range_ord_id]
        };
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_tm_bytes[..dif_tm_bytes_len as usize]);


        //      STAGE 5: flushing
        self.auto_flush();
    }


    #[inline(always)]
    pub fn event_instant(&mut self, hash: u32, string: &str) {
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
        if let Some(thread_info) = &mut self.local_packet_header.thread_info {
            thread_info.new_thread_name = Some(name);
        }
    }

    /// Check buffer length, and flush if the buffer is full
    #[inline(always)]
    pub fn auto_flush(&mut self) {
        if self.buf.len() >= self.config.flush_threshold {
            self.flush(false);
        }
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self, _finalize: bool) {
        let data = self.buf.clone();
        self.buf.clear();

        if data.is_empty() {
            // Nothing to flush, ignore
            return;
        }

        // Fill header
        self.local_packet_header.end_timestamp = self.prev_tm;
        self.local_packet_header.id_store = self.id_store.clone().into();

        self.global_storage_ref.flush(&self.local_packet_header, data);

        //cleanup
        if let Some(thread_info) = &mut self.local_packet_header.thread_info {
            if thread_info.new_thread_name.is_some() {
                thread_info.new_thread_name = None;
            }
        }
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
    range_start_id: u8, // required to create potentially new end event
    range_ord_id: u8, // required to match with start event during parsing

    _not_send: PhantomData<*const ()>
}