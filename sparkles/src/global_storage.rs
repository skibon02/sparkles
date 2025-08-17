//! Single global storage for sparkles events
//! All evens are being flushed into GLOBAL_STORAGE, and then head towards transport abstraction (UDP/TCP/file).

use std::io::Read;
use std::{mem, thread};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{JoinHandle};
use std::time::{Duration, Instant};
use log::{debug, error, trace, warn};
use parking_lot::{Condvar, Mutex};
use ringbuf::traits::{Consumer, Observer, Producer};
use smallvec::SmallVec;
use sparkles_core::{Timestamp, TimestampProvider};
use sparkles_core::protocol::headers::{LocalPacketHeader, SparklesMachineInfo};
use sparkles_core::protocol::packets::{send_external_event_names, send_external_events, send_external_sync_point, send_failed_pages, send_graceful_shutdown, send_machine_info, send_timestamp_freq, send_trace_data};
use sparkles_core::protocol::sender::{ConfiguredSender, Sender, SenderChain};
use sparkles_macro::static_name;
use crate::config::SparklesConfig;
use crate::{flush_thread_local, on_client_connect, GLOBAL_FLUSHING_RUNNING, THREAD_LOCAL_NOTIFICATION};
use crate::external_events::{EXTERNAL_EVENTS_NAMES, EXTERNAL_EVENTS_PACKETS, EXTERNAL_EVENTS_SYNC_POINTS};
use crate::monotonic::get_monotonic_nanos;
use crate::sender::file_sender::FileSender;
use crate::thread_local_storage::set_local_storage_config;

pub static GLOBAL_STORAGE: Mutex<Option<GlobalStorage>> = Mutex::new(None);
pub static TICKS_PER_MS: AtomicU32 = AtomicU32::new(0);
static FINALIZE_STARTED: AtomicBool = AtomicBool::new(false);
static SENDER_THREAD_ITERATION: Condvar = Condvar::new();

pub struct GlobalStorage {
    config: SparklesConfig,
    inner: ringbuf::LocalRb<ringbuf::storage::Heap<u8>>,
    sending_thread: Option<JoinHandle<()>>,

    // TODO: may grow as threads are spawned and finished
    range_end_requests: HashMap<u64, SmallVec<[u8; 8]>>,

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

            range_end_requests: HashMap::new(),

            skipped_msr_pages_headers: Vec::new(),
        }
    }


    /// Called by thread local storage to put its contents into global storage
    pub fn push_buf(&mut self, header: &LocalPacketHeader, buf: &[u8]) {
        // info!("Got new local buffer. start: {}, end: {}", header.start_timestamp, header.end_timestamp);
        let header = bincode::encode_to_vec(header, bincode::config::standard()).unwrap();
        let header_len = (header.len() as u64).to_be_bytes();
        let bufer_len = (buf.len() as u64).to_be_bytes();

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
                let header_len = u64::from_be_bytes(header_len) as usize;

                header_bytes.resize(header_len, 0);
                self.inner.read_exact(&mut header_bytes).unwrap();
                let (header, _) = bincode::decode_from_slice(&header_bytes, bincode::config::standard()).unwrap();

                self.inner.read_exact(&mut buf_len).unwrap();
                let buf_len = u64::from_be_bytes(buf_len) as usize;
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
            self.config.sending_threshold
        };
        if self.inner.occupied_len() > threshold {
            use crate as sparkles;
            #[cfg(feature="self-tracing")]
            let g = sparkles_macro::range_event_start!("[internal] Taking stored events");
            
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
    
    pub fn check_notify(&self) {
        let thr = self.config.sending_threshold;
        if self.inner.occupied_len() > thr {
            SENDER_THREAD_ITERATION.notify_one();
        }
    }
    pub fn exchange_closed_ranges(&mut self, thread_id: u64, closed_ranges: Vec<(u64, u8)>, mut incoming_closed_ranges: impl FnMut(&[u8])) {
        for (thread_id, range_id) in closed_ranges {
            self.range_end_requests.entry(thread_id).or_default().push(range_id);
        }
        if let Some(ends) = self.range_end_requests.get_mut(&thread_id) {
            incoming_closed_ranges(&mem::take(ends));
        }
    }
}

fn spawn_sending_task(config: SparklesConfig) -> JoinHandle<()> {
    thread::Builder::new()
        .name("[Sparkles] Sender thread".to_string())
        .spawn(move || {
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
        #[cfg(feature = "udp-streaming")]
        if let Some(udp_sender_config) = config.udp_sender_config.as_ref() {
            if let Some(sender) = crate::sender::udp_sender::UdpSender::new(udp_sender_config) {
                sender_chain.with_sender(sender);
            }
            else {
                warn!("[sparkles] Failed to create UDP sender!");
                on_client_connect();
            }
        }
        else {
            on_client_connect();
        }
        #[cfg(not(feature = "udp-streaming"))]
        on_client_connect();

        let process_name = std::env::current_exe().unwrap_or(PathBuf::from("main"))
            .file_name().unwrap().to_str().unwrap().to_string();
        let pid = std::process::id();

        let mut freq_detector = TimestampFreqDetector::start(Duration::from_millis(100));

        let info_header = SparklesMachineInfo::new(process_name, pid);
        send_machine_info(&mut sender_chain, info_header.clone());

        thread::sleep(Duration::from_millis(1));

        let (ticks_per_sec, cur_tm) = freq_detector.next_forced();
        send_timestamp_freq(&mut sender_chain, ticks_per_sec, cur_tm);

        let mut last_sender_poll_tm: Option<Instant> = None;
        let mut last_send_data_tm: Option<Instant> = None;

        let tmp_mutex = Mutex::new(());
        loop {
            use crate as sparkles;

            // senders polling
            if last_sender_poll_tm.is_none_or(|tm| tm.elapsed() > Duration::from_millis(200)) {
                sender_chain.poll();
                last_sender_poll_tm = Some(Instant::now());
            }

            // Timestamp freq and machine info packets
            if sender_chain.take_tm_freq_requested() {
                THREAD_LOCAL_NOTIFICATION.fetch_add(1, Ordering::Relaxed);

                let (ticks_per_sec, cur_tm) = freq_detector.next_forced();
                send_timestamp_freq(&mut sender_chain, ticks_per_sec, cur_tm);
                send_machine_info(&mut sender_chain, info_header.clone());
            }
            else if let Some((ticks_per_sec, cur_tm)) = freq_detector.next() {
                TICKS_PER_MS.store((ticks_per_sec / 1_000) as u32, Ordering::Relaxed);
                send_timestamp_freq(&mut sender_chain, ticks_per_sec, cur_tm);
            }

            // Read value before flushing
            let is_finalizing = FINALIZE_STARTED.load(Ordering::Relaxed);
            if is_finalizing {
                debug!("[sparkles] Finalize detected!");
            }

            let forced_flush = if let Some(last_send_data_tm) = last_send_data_tm {
                last_send_data_tm.elapsed() > Duration::from_millis(config.auto_send_ms as u64)
            } else {
                true
            };

            // this thing should be fast
            let (slices, failed_pages) = {
                #[cfg(feature="self-tracing")]
                if is_finalizing {
                    sparkles_macro::instant_event!("[internal] Finalizing");
                    flush_thread_local();
                }

                if let Some(global_storage) = GLOBAL_STORAGE.lock().as_mut() {
                    let failed_pages = global_storage.take_failed_pages();

                    GLOBAL_FLUSHING_RUNNING.store(true, Ordering::Relaxed);
                    (global_storage.try_take_buf(is_finalizing || forced_flush), failed_pages)
                }
                else {
                    (None, Vec::new())
                }
            };
            GLOBAL_FLUSHING_RUNNING.store(false, Ordering::Relaxed);

            // handle buffers
            if let Some((slice1, slice2)) = slices {
                #[cfg(feature="self-tracing")]
                let grd = crate::range_event_start(static_name!("[internal] Send data bytes"));
                send_trace_data(&mut sender_chain, &slice1, &slice2);
                last_send_data_tm = Some(Instant::now());
            }

            // handle failed pages
            if !failed_pages.is_empty() {
                trace!("Sending {} failed pages", failed_pages.len());
                send_failed_pages(&mut sender_chain, &failed_pages)
            }

            // External events
            if let Some(points) = mem::take(&mut *EXTERNAL_EVENTS_SYNC_POINTS.lock()) {
                for (local, external) in points {
                    send_external_sync_point(&mut sender_chain, local, external);
                }
            }

            if let Some(data) = mem::take(&mut *EXTERNAL_EVENTS_PACKETS.lock()) {
                for (header, data) in data {
                    send_external_events(&mut sender_chain, header, data);
                }
            }

            if let Some(names) = mem::take(&mut *EXTERNAL_EVENTS_NAMES.lock()) {
                for names_packet in names {
                    send_external_event_names(&mut sender_chain, names_packet);
                }
            }

            if is_finalizing {
                debug!("[internal] Finalize in process...");
                send_graceful_shutdown(&mut sender_chain);
                break;
            }

            if !FINALIZE_STARTED.load(Ordering::Relaxed) {
                let mut mutex = tmp_mutex.lock();
                #[cfg(feature="self-tracing")]
                let g = sparkles_macro::range_event_start!("[internal] Waiting for signal");
                if SENDER_THREAD_ITERATION.wait_for(&mut mutex, Duration::from_millis(50)).timed_out() {
                    #[cfg(feature="self-tracing")]
                    sparkles_macro::range_event_end!(g, "Timeout!");
                }
            }
        }

        debug!("[sparkles] Quit from flush thread!");
    }).unwrap()
}

/// Blocking wait for global sending thread to finish its job
pub fn finalize() {
    use crate as sparkles;
    #[cfg(feature="self-tracing")]
    sparkles_macro::instant_event!("[internal] Finalize requested");
    
    // Flush current thread
    flush_thread_local();

    FINALIZE_STARTED.store(true, Ordering::SeqCst);
    SENDER_THREAD_ITERATION.notify_one();
    let jh = if let Some(global_storage) = GLOBAL_STORAGE.lock().as_mut() {
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
    prev_monotonic: u64,

    capture_interval_ns: u64,
}

impl TimestampFreqDetector {
    pub fn start(interval: Duration) -> Self {
        let now = get_monotonic_nanos();
        let now_tm = Timestamp::now();
        Self {
            prev_monotonic: now,
            prev_tm: now_tm,

            capture_interval_ns: interval.as_nanos() as u64,
        }
    }
    pub fn next(&mut self) -> Option<(u64, u64)> {
        if get_monotonic_nanos() - self.prev_monotonic > self.capture_interval_ns {
            Some(self.next_forced())
        }
        else {
            None
        }
    }

    pub fn next_forced(&mut self) -> (u64, u64) {
        let now = get_monotonic_nanos();
        let now_tm = Timestamp::now();

        let elapsed_tm = now_tm.wrapping_sub(self.prev_tm) as f64;
        let elapsed_ns = (now - self.prev_monotonic) as f64;
        let ticks_per_sec = elapsed_tm / elapsed_ns.max(1.0) * 1_000_000_000.0;

        self.prev_tm = now_tm;
        self.prev_monotonic = now;

        (ticks_per_sec as u64, now_tm)
    }
}