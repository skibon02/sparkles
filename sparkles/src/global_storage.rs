//! Single global storage for sparkles events
//! All evens are being flushed into GLOBAL_STORAGE, and then head towards transport abstraction (UDP/TCP/file).

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Mutex;
use std::{mem, thread};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle};
use std::time::Duration;
use log::{debug, error, trace, warn};
use ringbuf::traits::{Consumer, Observer, Producer};
use sparkles_core::headers::{LocalPacketHeader, ThreadNameHeader};


/// Preallocate 500MB for trace buffer
pub const GLOBAL_CAPACITY: usize = 500*1024*1024;

pub const CLEANUP_THRESHOLD: usize = (GLOBAL_CAPACITY as f64 * 0.9) as usize;
pub const CLEANUP_BOTTOM_THRESHOLD: usize = 350*1024*1024;
pub const FLUSH_THRESHOLD: usize = 5*1024*1024;

pub static GLOBAL_STORAGE: Mutex<Option<GlobalStorage>> = Mutex::new(None);
static FINALIZE_STARTED: AtomicBool = AtomicBool::new(false);

pub struct GlobalStorage {
    inner: ringbuf::LocalRb<ringbuf::storage::Heap<u8>>,
    sending_thread: Option<JoinHandle<()>>,

    skipped_msr_pages_headers: Vec<LocalPacketHeader>,
    thread_name_headers: Vec<ThreadNameHeader>,
}

impl Default for GlobalStorage {
    fn default() -> Self {
        let jh = thread::spawn(|| {
            debug!("[sparkles] Flush thread started! Connecting to remote...");
            let mut con = TcpStream::connect("127.0.0.1:4302").unwrap();
            debug!("[sparkles] Connected!");

            loop {
                // TODO: replace sleep with waiting for finalize
                thread::sleep(Duration::from_millis(20));

                let is_finalizing = FINALIZE_STARTED.load(Ordering::Relaxed);
                if is_finalizing {
                    debug!("[sparkles] Finalize detected!");
                }

                // this thing should be fast
                let (slices, failed_pages, thread_names) = {
                    if let Some(global_storage) = GLOBAL_STORAGE.lock().unwrap().as_mut() {
                        let failed_pages = global_storage.take_failed_pages();
                        let thread_names = global_storage.take_thread_names();

                        (global_storage.try_take_buf(is_finalizing), failed_pages, thread_names)
                    }
                    else {
                        (None, Vec::new(), Vec::new())
                    }
                };

                // handle failed pages
                if thread_names.len() > 0 {
                    debug!("Sending {} thread names", thread_names.len());
                    for thread_name_header in thread_names {
                        let header = bincode::serialize(&thread_name_header).unwrap();
                        let header_len = (header.len() as u64).to_le_bytes();
                        con.write_all(&[0x03]).unwrap();
                        con.write_all(&header_len).unwrap();
                        con.write_all(&header).unwrap();
                    }
                }
                
                // handle buffers
                if let Some((slice1, slice2)) = slices {
                    trace!("[sparkles] took two fresh slices! sizes: {}, {}", slice1.len(), slice2.len());
                    con.write_all(&[0x01]).unwrap();
                    let total_len = (slice1.len() + slice2.len()) as u64;
                    let total_len_bytes = total_len.to_le_bytes();
                    con.write_all(&total_len_bytes).unwrap();
                    con.write_all(&slice1).unwrap();
                    con.write_all(&slice2).unwrap();
                }

                // handle failed pages
                if failed_pages.len() > 0 {
                    trace!("Sending {} failed pages", failed_pages.len());
                    for failed_msr_page in failed_pages {
                        let header = bincode::serialize(&failed_msr_page).unwrap();
                        let header_len = (header.len() as u64).to_le_bytes();
                        con.write_all(&[0x02]).unwrap();
                        con.write_all(&header_len).unwrap();
                        con.write_all(&header).unwrap();
                    }
                }

                if is_finalizing {
                    debug!("[sparkles] Finalize in process...");
                    con.write_all(&[0xff]).unwrap();
                    con.shutdown(Shutdown::Both).unwrap();
                    break;
                }
            }

            debug!("[sparkles] Quit from flush thread!");
        });


        Self {
            inner: ringbuf::LocalRb::new(GLOBAL_CAPACITY),
            sending_thread: Some(jh),

            thread_name_headers: Vec::new(),
            skipped_msr_pages_headers: Vec::new(),
        }
    }

}

impl GlobalStorage {
    /// Called by thread local storage to put its contents into global storage
    pub fn push_buf(&mut self, header: LocalPacketHeader, buf: &[u8]) {
        // info!("Got local packet: {:?}", header);
        let header = bincode::serialize(&header).unwrap();
        let header_len = (header.len() as u64).to_le_bytes();
        let bufer_len = (buf.len() as u64).to_le_bytes();

        self.inner.push_slice(&header_len);
        self.inner.push_slice(&header);
        self.inner.push_slice(&bufer_len);
        self.inner.push_slice(&buf);

        if self.inner.occupied_len() > CLEANUP_THRESHOLD {
            // self.dump_sizes();
            warn!("[sparkles] BUFFER FULL! clearing...");
            let mut header_len = [0u8; 8];
            let mut buf_len = [0u8; 8];
            let mut header_bytes = Vec::new();
            while self.inner.occupied_len() > CLEANUP_BOTTOM_THRESHOLD {
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
            // self.dump_sizes();
        }
    }

    /// Called by thread local storage to put new thread info header
    pub fn update_thread_name(&mut self, thread_name_header: ThreadNameHeader) {
        self.thread_name_headers.push(thread_name_header);
    }

    fn take_failed_pages(&mut self) -> Vec<LocalPacketHeader> {
        mem::take(&mut self.skipped_msr_pages_headers)
    }

    fn take_thread_names(&mut self) -> Vec<ThreadNameHeader> {
        mem::take(&mut self.thread_name_headers)
    }

    fn try_take_buf(&mut self, take_everything: bool) -> Option<(Vec<u8>, Vec<u8>)> {
        let threshold = if take_everything {
            0
        } else {
            FLUSH_THRESHOLD
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

    // fn dump_sizes(&self)  {
    //     debug!("\n\n\t*** STORAGE METRICS DUMP***");
    //     info!("Occupied len: {}", self.inner.occupied_len());
    //     info!("Skipped pages count: {}", self.skipped_msr_pages_headers.len());
    //     info!("");
    // }

    fn take_jh(&mut self) -> Option<JoinHandle<()>> {
        self.sending_thread.take()
    }
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