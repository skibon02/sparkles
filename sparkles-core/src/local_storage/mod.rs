use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::config::LocalStorageConfig;
use crate::local_storage::id_mapping::{EventType, IdMappingState};
use crate::protocol::headers::{LocalPacketHeader, ThreadInfo};
use crate::Timestamp;

use crate::timestamp::TimestampProvider;

pub mod id_mapping;

pub trait GlobalStorageImpl {
    fn flush(&self, header: &LocalPacketHeader, data: &[u8]);
    fn try_flush(&self, header: &LocalPacketHeader, data: &[u8]) -> bool;
    fn is_buf_available(&self) -> bool;
    fn take_new_update(&mut self) -> bool;
    fn exchange_closed_ranges(&mut self, thread_id: u64, closed_ranges: Vec<(u64, u8)>, incoming_closed_ranges: impl FnMut(&[u8]));
    fn ticks_per_ms(&self) -> u32;
}

pub struct LocalStorage<G: GlobalStorageImpl> {
    config: LocalStorageConfig,
    
    prev_tm: u64,

    buf: Vec<u8>,
    id_store: IdMappingState,

    local_packet_header: LocalPacketHeader,

    global_storage_ref: G,
    last_range_ord_id: u8,
    
    started_ranges: [bool; 256],
    started_ranges_cnt: usize,
    foreign_thread_ended: Vec<(u64, u8)>, // (thread_id, range_ord_id)

    prev_flush_tm: u64,

    flush_event_hash: u32,
    flush_event_str: &'static str,

    thread_name: Option<String>,
}

static CUR_THREAD_ID: AtomicUsize = AtomicUsize::new(1);

impl<G: GlobalStorageImpl> LocalStorage<G> {
    pub fn new(global_storage_ref: G, thread_info: ThreadInfo, config: LocalStorageConfig)-> Self {
        let thread_ord_id = CUR_THREAD_ID.fetch_add(1, Ordering::Relaxed) as u64;

        let thread_name = thread_info.new_thread_name.clone();
        let flush_event_str = "[sparkles] Flushing local storage";
        let flush_event_hash = sparkles_macro::calc_hash!("[sparkles] Flushing local storage");

        let now_tm = Timestamp::now();
        LocalStorage {
            config,
            buf: Vec::new(),
            prev_tm: now_tm,

            id_store: Default::default(),
            local_packet_header: LocalPacketHeader {
                thread_ord_id,
                thread_info,

                ..Default::default()
            },

            global_storage_ref,
            last_range_ord_id: 0,
            started_ranges: [false; 256],
            started_ranges_cnt: 0,
            foreign_thread_ended: Vec::new(),

            prev_flush_tm: now_tm,

            flush_event_hash,
            flush_event_str,

            thread_name,
        }
    }

    fn new_range_ord_id(&mut self) -> u8 {
        let range_ord_id = self.last_range_ord_id;
        self.last_range_ord_id = self.last_range_ord_id.wrapping_add(1);
        if self.started_ranges_cnt >= 256 {
            self.started_ranges_cnt += 1;
            range_ord_id
        }
        else {
            // Guaranteed to find a free range_ord_id slot
            while self.started_ranges[self.last_range_ord_id as usize] {
                self.last_range_ord_id = self.last_range_ord_id.wrapping_add(1);
            }
            
            self.started_ranges[range_ord_id as usize] = true;
            self.started_ranges_cnt += 1;
            range_ord_id
        }
    }

    #[inline(always)]
    pub fn event_range_start(&mut self, hash: u32, name: &str) -> RangeStartRepr {
        self.event_range_start_inner(hash, name, false)
    }

    fn event_range_start_inner(&mut self, hash: u32, name: &str, prevent_flushing: bool) -> RangeStartRepr {
        // On a new range event we acquire new range_ord_id to match start and end events
        let range_ord_id = self.new_range_ord_id();
        let start_id = self.id_store.insert_and_get_id(hash, name, EventType::RangeStart);
        self.range_event(Some(start_id), range_ord_id, prevent_flushing, None);

        RangeStartRepr {
            range_ord_id,
            range_start_id: start_id,

            start_thread_id: self.local_packet_header.thread_ord_id,
        }
    }

    #[inline(always)]
    pub fn event_range_end(&mut self, range_start: RangeStartRepr, hash: u32, name: &str) {
        self.event_range_end_inner(range_start, hash, name, false);
    }

    #[inline(always)]
    fn event_range_end_inner(&mut self, range_start: RangeStartRepr, hash: u32, name: &str, prevent_flushing: bool) {
        let range_ord_id = range_start.range_ord_id;
        let foreign_thread_id = if range_start.start_thread_id != self.local_packet_header.thread_ord_id {
            // Foreign range end event. We should notify the global storage about it.
            self.foreign_thread_ended.push((range_start.start_thread_id, range_ord_id));
            Some(range_start.start_thread_id)
        }
        else {
            self.started_ranges[range_start.range_ord_id as usize] = false;
            self.started_ranges_cnt -= 1;
            None
        };
        let start_id = range_start.range_start_id;
        let event_id = if hash != 0 {
            let end_id = self.id_store.insert_and_get_id(hash, name, EventType::RangeEnd(start_id));
            Some(end_id)
        }
        else {
            None
        };
        self.range_event(event_id, range_ord_id, prevent_flushing, foreign_thread_id);
    }

    #[inline(always)]
    fn range_event(&mut self, id: Option<u8>, range_ord_id: u8, prevent_flushing: bool, foreign_thread_id: Option<u64>) {
        //      STAGE 2: Acquire timestamp and calculate now, dif_tm
        //    (3ns on non-serializing x86 timestamp, 11ns on serializing x86 timestamp)
        let timestamp = Timestamp::now();

        //      STAGE 3: Update local info
        let dif_tm = self.update_local_info(timestamp);

        //      STAGE 4: PUSH VALUES
        let dif_tm_bytes: [u8; 8] = dif_tm.to_le_bytes();
        let dif_tm_bytes_len = ((Timestamp::TIMESTAMP_VALID_BITS as u32 + 7 - dif_tm.leading_zeros()) >> 3) as u8;
        let mut buf = match id {
            Some(id) => [id, dif_tm_bytes_len | 0x80, range_ord_id], // Range flag
            None => [0, dif_tm_bytes_len | 0xC0, range_ord_id] // Range + UnnamedEnd flags
        };
        if foreign_thread_id.is_some() {
            buf[1] |= 0x20; // Set foreign thread flag
        }
        self.buf.extend_from_slice(&buf);
        self.buf.extend_from_slice(&dif_tm_bytes[..dif_tm_bytes_len as usize]);
        if let Some(foreign_thread_id) = foreign_thread_id {
            let foreign_thread_id_bytes: [u8; 8] = foreign_thread_id.to_le_bytes();
            let foreign_thread_id_bytes_len = ((64 + 7 - foreign_thread_id.leading_zeros()) >> 3) as u8;
            self.buf.push(foreign_thread_id_bytes_len);
            self.buf.extend_from_slice(&foreign_thread_id_bytes[..foreign_thread_id_bytes_len as usize]);
        }


        //      STAGE 5: flushing
        if !prevent_flushing {
            self.auto_flush();
        }
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
        self.thread_name = Some(name);
        self.local_packet_header.thread_info.new_thread_name = self.thread_name.clone();
    }

    /// Check buffer length, and flush if the buffer is full
    #[inline(always)]
    pub fn auto_flush(&mut self) {
        let ticks_per_ms = self.global_storage_ref.ticks_per_ms();
        if self.buf.len() >= self.config.flush_threshold {
            self.flush(true);
        }
        else if self.buf.len() >= self.config.flush_attempt_threshold && self.global_storage_ref.is_buf_available() {
            self.flush(false);
        }
        else if ticks_per_ms != 0 && Timestamp::now() - self.prev_flush_tm > self.config.auto_flush_ms as u64 * ticks_per_ms as u64 && self.global_storage_ref.is_buf_available() {
            self.flush(false);
        }
    }

    /// Flush whole event buffer data to the global storage
    pub fn flush(&mut self, blocking: bool) {
        if self.buf.is_empty() {
            // Nothing to flush, ignore
            return;
        }

        self.prev_flush_tm = Timestamp::now();

        #[cfg(feature = "self-tracing")]
        let range_event = self.event_range_start_inner(self.flush_event_hash, self.flush_event_str, true);
        let new_update = self.global_storage_ref.take_new_update();
        if new_update {
            self.local_packet_header.thread_info.new_thread_name = self.thread_name.clone();
        }

        // Exchange closed ranges
        self.global_storage_ref.exchange_closed_ranges(
            self.local_packet_header.thread_ord_id,
            core::mem::take(&mut self.foreign_thread_ended),
            |closed_ranges| {
                for &rng in closed_ranges {
                    self.started_ranges[rng as usize] = false;
                    self.started_ranges_cnt -= 1;
                }
            }
        );

        // Fill header
        self.local_packet_header.end_timestamp = self.prev_tm;
        self.local_packet_header.id_store = self.id_store.clone().into();

        let success = if blocking {
            self.global_storage_ref.flush(&self.local_packet_header, &self.buf);
            true
        }
        else {
            self.global_storage_ref.try_flush(&self.local_packet_header, &self.buf)
        };

        //cleanup
        if success {
            self.buf.clear();
            if self.local_packet_header.thread_info.new_thread_name.is_some() {
                self.local_packet_header.thread_info.new_thread_name = None;
            }
            self.local_packet_header.start_timestamp = 0;
        }
        #[cfg(feature = "self-tracing")]
        self.event_range_end_inner(range_event, 0, "", true);
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

    start_thread_id: u64,
}