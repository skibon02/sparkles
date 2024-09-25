//! Single global storage for sparkles events
//! All evens are being flushed into GLOBAL_STORAGE, and then head towards transport abstraction (UDP/TCP/file).

use std::io::{Read, Write};
use std::sync::Mutex;
use std::{fs, mem, thread};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle};
use std::time::Duration;
use log::{debug, error, trace, warn};
use ringbuf::traits::{Consumer, Observer, Producer};
use sparkles_core::config::SparklesConfig;
use sparkles_core::headers::{LocalPacketHeader, SparklesEncoderInfo};
use sparkles_core::{Timestamp, TimestampProvider};

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
        let jh = spawn_sending_task();

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

fn spawn_sending_task() -> JoinHandle<()> {
    thread::spawn(|| {
        debug!("[sparkles] Flush thread started!");

        // 1. Create log file
        let dir = "trace";
        if !fs::metadata(dir).is_ok() {
            debug!("[sparkles] Creating output directory...");
            fs::create_dir(dir).unwrap();
        }

        let now = chrono::Local::now();
        let filename = format!("{}/{}.sprk", dir, now.format("%Y-%m-%d_%H-%M-%S"));
        debug!("[sparkles] Creating output file: {}", filename);
        let mut file = fs::File::create(filename).unwrap();

        let process_name = std::env::current_exe().unwrap().file_name().unwrap().to_str().unwrap().to_string();
        let pid = std::process::id();

        let info_header = SparklesEncoderInfo {
            ver: sparkles_core::consts::ENCODER_VERSION,
            counts_per_ns: Timestamp::COUNTS_PER_NS,
            process_name,
            pid
        };
        let encoded_info = bincode::serialize(&info_header).unwrap();

        file.write_all(&[0x00]).unwrap();
        file.write_all(&(encoded_info.len() as u64).to_le_bytes()).unwrap();
        file.write_all(&encoded_info).unwrap();

        loop {
            thread::sleep(Duration::from_millis(1));

            // Read value before flushing
            let is_finalizing = FINALIZE_STARTED.load(Ordering::Relaxed);
            if is_finalizing {
                debug!("[sparkles] Finalize detected!");
            }

            // this thing should be fast
            let (slices, failed_pages) = {
                if let Some(global_storage) = GLOBAL_STORAGE.lock().unwrap().as_mut() {
                    let failed_pages = global_storage.take_failed_pages();

                    (global_storage.try_take_buf(is_finalizing), failed_pages)
                }
                else {
                    (None, Vec::new())
                }
            };

            // handle buffers
            if let Some((slice1, slice2)) = slices {
                trace!("[sparkles] took two fresh slices! sizes: {}, {}", slice1.len(), slice2.len());
                file.write_all(&[0x01]).unwrap();
                let total_len = (slice1.len() + slice2.len()) as u64;
                let total_len_bytes = total_len.to_le_bytes();
                file.write_all(&total_len_bytes).unwrap();
                file.write_all(&slice1).unwrap();
                file.write_all(&slice2).unwrap();
            }

            // handle failed pages
            if !failed_pages.is_empty() {
                trace!("Sending {} failed pages", failed_pages.len());
                for failed_msr_page in failed_pages {
                    let header = bincode::serialize(&failed_msr_page).unwrap();
                    let header_len = (header.len() as u64).to_le_bytes();
                    file.write_all(&[0x02]).unwrap();
                    file.write_all(&header_len).unwrap();
                    file.write_all(&header).unwrap();
                }
            }

            if is_finalizing {
                debug!("[sparkles] Finalize in process...");
                file.write_all(&[0xff]).unwrap();
                break;
            }
        }

        debug!("[sparkles] Quit from flush thread!");
    })
}

/// Wait for TCP sending thread to finish its job
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