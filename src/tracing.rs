use std::hint::black_box;
use std::{mem, thread};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use log::{info, warn};

pub static IS_FINISHED: AtomicBool = AtomicBool::new(false);
pub static FAILED_LOCKS: AtomicUsize = AtomicUsize::new(0);

pub trait SharedTraceBufferTrait {
    fn try_push(&self, v: &[u8]) -> Option<()>;
    fn try_pop<const N: u8>(&self) -> Option<[u8; N as usize]>;
    fn new() -> Self;
}

fn recv_thread<F: SharedTraceBufferTrait + Sync + Send + 'static>(rx: Arc<F>) -> JoinHandle<()> {
    let mut accum: u64 = 0;
    thread::spawn(move || {
        while !IS_FINISHED.load(Ordering::Relaxed) {
            let bytes = rx.try_pop::<50>();
            if bytes.is_some() {
                accum += 50;
            }
            else {
                FAILED_LOCKS.fetch_add(1, Ordering::Relaxed);
            }
            // std::thread::sleep(Duration::from_nanos(1));
            thread::yield_now();
        }
        info!("Finished! Total received bytes: {}", accum);
        info!("Failed try_pop calls: {}", FAILED_LOCKS.load(Ordering::Relaxed));
    })
}

pub struct Tracer<F: SharedTraceBufferTrait> {
    handle: Option<JoinHandle<()>>,
    ringbuf: Arc<F>,
    start: SystemTime,

    // 24 bits are significant
    prev_period: AtomicU32,
}

impl<F: SharedTraceBufferTrait + Send + Sync + 'static> Tracer<F> {
    pub fn new() -> Self {
        IS_FINISHED.store(false, Ordering::Relaxed);
        let now = SystemTime::now();

        let ringbuf = Arc::new(F::new());

        let handle = recv_thread(ringbuf.clone());
        Self {
            ringbuf,
            handle: Some(handle),
            start: now,
            prev_period: AtomicU32::new(0),
        }
    }
    pub fn event(&self, v: u8) {
        let (mut dif_pr, now) = black_box(self.capture_timestamp());
        let mut buf = [0; 20];
        buf[0] = v | 0x80;
        buf[1] = now;

        let mut ind = 2;
        // While value is 32 bits, we send 6 bits at a time
        while dif_pr > 0 {
            let is_last = (dif_pr < 0x3F) as u8;
            let cur_dif = dif_pr as u8 & 0x3F;
            buf[ind] = cur_dif | (is_last << 6);

            dif_pr >>= 6;
            ind += 1;
        }

        // Write event packet
        if self.ringbuf.try_push(&buf[..ind]).is_none() {
            warn!("Failed to push event packet into buffer!");
        }
    }

    /// returns dif_pr and 8 last bits of timestamp
    fn capture_timestamp(&self) -> (u32, u8) {
        let mut prev_pr = self.prev_period.load(Ordering::Relaxed);
        loop {
            let now = self.start.elapsed().unwrap().as_nanos() as u64; // takes 24ns
            // let now = 8234721;
            let now_pr = (now >> 8) as u32;
            let dif_pr = now_pr.saturating_sub(prev_pr);
            if dif_pr > 0 {
                match self.prev_period.compare_exchange(prev_pr, now_pr, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => {
                        return (dif_pr, now as u8);
                    },
                    Err(x) => prev_pr = x
                }
            }
            else {
                return (0, now as u8);
            }
        }
        // (1, 27)
    }
}

impl<F: SharedTraceBufferTrait> Drop for Tracer<F> {
    fn drop(&mut self) {
        let _ = mem::replace(&mut self.ringbuf, Arc::new(F::new()));
        self.handle.take().unwrap().join().unwrap()
    }
}