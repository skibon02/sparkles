use core::hint::black_box;
use core::mem;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use core::time::Duration;
use log::{info, warn};

pub trait SharedTraceBufferTrait: Send + Sync + 'static {
    fn try_push(&self, v: &[u8]) -> Option<()>;
    fn try_pop<const N: u8>(&self) -> Option<[u8; N as usize]>;
    fn new() -> Self;
}

pub trait TraceCollector<F: SharedTraceBufferTrait> {
    fn spawn(ringbuf: Arc<F>, is_finished: Arc<AtomicBool>) -> Self;
    fn wait_for_finish(self);
}

pub trait TimestampImpl {
    fn now() -> Self;
    fn elapsed_ns(&self) -> u64;
}

pub struct Tracer<F: SharedTraceBufferTrait, I: TraceCollector<F>, T: TimestampImpl> {
    ringbuf: Arc<F>,
    start: T,
    trace_collector: Option<I>,

    // 24 bits are significant
    prev_period: AtomicU32,

    is_finished: Arc<AtomicBool>
}

impl<F: SharedTraceBufferTrait + Send + Sync + 'static, I, T> Tracer<F, I, T>
    where I: TraceCollector<F>, T: TimestampImpl {
    pub fn new() -> Self {
        let now = T::now();

        let ringbuf = Arc::new(F::new());
        let is_finished = Arc::new(AtomicBool::new(false));

        let trace_collector = Some(I::spawn(ringbuf.clone(), is_finished.clone()));
        Self {
            ringbuf,
            start: now,
            prev_period: AtomicU32::new(0),
            trace_collector,
            is_finished
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
            let now = self.start.elapsed_ns();
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

impl<F: SharedTraceBufferTrait, I, T> Drop for Tracer<F, I, T>
    where I: TraceCollector<F>, T: TimestampImpl {
    fn drop(&mut self) {
        self.is_finished.store(true, Ordering::Relaxed);
        let _ = mem::replace(&mut self.ringbuf, Arc::new(F::new()));
        self.trace_collector.take().unwrap().wait_for_finish();
    }
}