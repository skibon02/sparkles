//! Single global storage for sparkles events
//! All evens are being flushed into GLOBAL_STORAGE, and then head towards transport abstraction (UDP/TCP/file).

use std::io::{Read, Write};
use std::sync::Mutex;
use std::{mem, thread};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle};
use std::time::{Duration, Instant};
use log::{debug, error, info, trace, warn};
use ringbuf::traits::{Consumer, Observer, Producer};
use sparkles_core::headers::{LocalPacketHeader, SparklesEncoderInfo};
use sparkles_core::{Timestamp, TimestampProvider};
use sparkles_core::sender::{ConfiguredSender, Sender, SenderChain};
use crate::config::SparklesConfig;
use crate::encoder::{send_data_bytes, send_encoder_info_packet, send_failed_page_headers, send_timestamp_freq};
use crate::GLOBAL_FLUSHING_RUNNING;
use crate::sender::file_sender::FileSender;
use crate::thread_local_storage::set_local_storage_config;

pub static GLOBAL_STORAGE: Mutex<Option<GlobalStorage>> = Mutex::new(None);
static FINALIZE_STARTED: AtomicBool = AtomicBool::new(false);

pub struct GlobalStorage {
    config: SparklesConfig,
    inner: ringbuf::LocalRb<ringbuf::storage::Heap<u8>>,
    sending_thread: Option<JoinHandle<()>>,

    skipped_msr_pages_headers: Vec<LocalPacketHeader>,
}

impl GlobalStorage {
    /// Create new global storage with given config and spawn sending thread
    pub fn new(config: SparklesConfig) -> Self {
        // Set local storage config
        set_local_storage_config(config.local_storage_config);

        let jh = spawn_sending_task(config.clone());

        let global_capacity = config.global_capacity;
        Self {
            config,
            inner: ringbuf::LocalRb::new(global_capacity),
            sending_thread: Some(jh),

            skipped_msr_pages_headers: Vec::new(),
        }
    }


    /// Called by thread local storage to put its contents into global storage
    pub fn push_buf(&mut self, header: &LocalPacketHeader, buf: &[u8]) {
        let header = bincode::serialize(&header).unwrap();
        let header_len = (header.len() as u64).to_le_bytes();
        let bufer_len = (buf.len() as u64).to_le_bytes();

        self.inner.push_slice(&header_len);
        self.inner.push_slice(&header);
        self.inner.push_slice(&bufer_len);
        self.inner.push_slice(buf);

        if self.inner.occupied_len() > (self.config.cleanup_threshold * self.config.global_capacity as f64) as usize {
            warn!("[sparkles] BUFFER FULL! starting cleanup..");
            let mut header_len = [0u8; 8];
            let mut buf_len = [0u8; 8];
            let mut header_bytes = Vec::new();
            while self.inner.occupied_len() > (self.config.cleanup_bottom_threshold * self.config.global_capacity as f64) as usize {
                self.inner.read_exact(&mut header_len).unwrap();
                let header_len = u64::from_le_bytes(header_len) as usize;

                header_bytes.resize(header_len, 0);
                self.inner.read_exact(&mut header_bytes).unwrap();
                let header = bincode::deserialize::<LocalPacketHeader>(&header_bytes).unwrap();

                self.inner.read_exact(&mut buf_len).unwrap();
                let buf_len = u64::from_le_bytes(buf_len) as usize;
                self.inner.skip(buf_len);
                self.skipped_msr_pages_headers.push(header);
            }
        }
    }

    fn take_failed_pages(&mut self) -> Vec<LocalPacketHeader> {
        mem::take(&mut self.skipped_msr_pages_headers)
    }

    fn try_take_buf(&mut self, take_everything: bool) -> Option<(Vec<u8>, Vec<u8>)> {
        let threshold = if take_everything {
            0
        } else {
            (self.config.flush_threshold * self.config.global_capacity as f64) as usize
        };
        if self.inner.occupied_len() > threshold {
            debug!("[sparkles] Flushing..");
            let slices = self.inner.as_slices();
            let slices = (slices.0.to_vec(), slices.1.to_vec());
            self.inner.clear();
            Some(slices)
        }
        else {
            None
        }
    }

    fn take_jh(&mut self) -> Option<JoinHandle<()>> {
        self.sending_thread.take()
    }
}

fn spawn_sending_task(config: SparklesConfig) -> JoinHandle<()> {
    thread::Builder::new().name("[Sparkles] Sender thread".to_string()).spawn(move || {
        debug!("[sparkles] Flush thread started!");

        let mut sender_chain = SenderChain::default();
        if let Some(file_sender_config) = config.file_sender_config.as_ref() {
            if let Some(sender) = FileSender::new(file_sender_config) {
                sender_chain.with_sender(sender);
            }
            else {
                warn!("[sparkles] Failed to create file sender!");
            }
        }
        if let Some(udp_sender_config) = config.udp_sender_config.as_ref() {
            if let Some(sender) = crate::sender::udp_sender::UdpSender::new(udp_sender_config) {
                sender_chain.with_sender(sender);
            }
            else {
                warn!("[sparkles] Failed to create UDP sender!");
            }
        }

        let process_name = std::env::current_exe().unwrap().file_name().unwrap().to_str().unwrap().to_string();
        let pid = std::process::id();

        let mut freq_detector = TimestampFreqDetector::start(Duration::from_millis(100));

        let info_header = SparklesEncoderInfo::new(process_name, pid);
        send_encoder_info_packet(&mut sender_chain, info_header);

        loop {
            thread::sleep(Duration::from_millis(1));

            if let Some(ticks_per_sec) = freq_detector.next() {
                send_timestamp_freq(&mut sender_chain, ticks_per_sec);
            }

            // Read value before flushing
            let is_finalizing = FINALIZE_STARTED.load(Ordering::Relaxed);
            if is_finalizing {
                debug!("[sparkles] Finalize detected!");
            }

            // this thing should be fast
            let (slices, failed_pages) = {
                if is_finalizing {
                    crate::flush_thread_local();
                }

                if let Some(global_storage) = GLOBAL_STORAGE.lock().unwrap().as_mut() {
                    #[cfg(feature="self-tracing")]
                    let grd = crate::range_event_start(crate::calculate_hash("[internal] Taking stored events"), "[internal] Taking stored events");
                    let failed_pages = global_storage.take_failed_pages();
                    
                    GLOBAL_FLUSHING_RUNNING.store(true, Ordering::Relaxed);
                    (global_storage.try_take_buf(is_finalizing), failed_pages)
                }
                else {
                    (None, Vec::new())
                }
            };
            GLOBAL_FLUSHING_RUNNING.store(false, Ordering::Relaxed);

            // handle buffers
            if let Some((slice1, slice2)) = slices {
                #[cfg(feature="self-tracing")]
                let grd = crate::range_event_start(crate::calculate_hash("[internal] Send data bytes"), "[internal] Send data bytes");
                send_data_bytes(&mut sender_chain, &slice1, &slice2);
            }

            // handle failed pages
            if !failed_pages.is_empty() {
                trace!("Sending {} failed pages", failed_pages.len());
                send_failed_page_headers(&mut sender_chain, &failed_pages)
            }

            if is_finalizing {
                let ticks_per_sec = freq_detector.next_forced();
                send_timestamp_freq(&mut sender_chain, ticks_per_sec);
                
                debug!("[sparkles] Finalize in process...");
                sender_chain.send(&[0xff]);
                break;
            }
        }

        debug!("[sparkles] Quit from flush thread!");
    }).unwrap()
}

/// Blocking wait for global sending thread to finish its job
pub fn finalize() {
    super::flush_thread_local();

    FINALIZE_STARTED.store(true, Ordering::Relaxed);
    let jh = if let Some(global_storage) = GLOBAL_STORAGE.lock().unwrap().as_mut() {
        global_storage.take_jh()
    } else {
        None
    };

    if let Some(jh) = jh {
        debug!("[sparkles] Joining sparkles flush thread...");
        let _ = jh.join().inspect_err(|e| {
            error!("Error while joining sparkles' flush thread! {:?}", e);
        });
    }

}

struct TimestampFreqDetector {
    prev_tm: u64,
    prev_instant: Instant,

    capture_interval: Duration,
}

impl TimestampFreqDetector {
    pub fn start(interval: Duration) -> Self {
        let now = Instant::now();
        let now_tm = Timestamp::now();
        Self {
            prev_instant: now,
            prev_tm: now_tm,

            capture_interval: interval,
        }
    }
    pub fn next(&mut self) -> Option<u64> {
        if self.prev_instant.elapsed() > self.capture_interval {
            Some(self.next_forced())
        }
        else {
            None
        }
    }

    pub fn next_forced(&mut self) -> u64 {

        let now = Instant::now();
        let now_tm = Timestamp::now();

        let elapsed_tm = now_tm.wrapping_sub(self.prev_tm) as f64;
        let elapsed_ns = (now - self.prev_instant).as_nanos() as f64;
        let ticks_per_sec = elapsed_tm / elapsed_ns * 1_000_000_000.0;

        self.prev_tm = now_tm;
        self.prev_instant = now;

        ticks_per_sec as u64
    }
}