use std::{mem, thread};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::SystemTime;
use log::info;
use crate::fifo::AtomicTimestampsRing;

pub static IS_FINISHED: AtomicBool = AtomicBool::new(false);

fn recv_thread(rx: Arc<AtomicTimestampsRing>) -> JoinHandle<()> {
    let mut accum: u64 = 0;
    thread::spawn(move || {
        while !IS_FINISHED.load(Ordering::Relaxed) {
            let byte = rx.try_pop::<50>();
            if byte.is_some() {
                accum += 50;
            }
        }
        info!("Finished! Total received bytes: {}", accum);
    })
}

pub struct Tracer {
    handle: Option<JoinHandle<()>>,
    ringbuf: Arc<AtomicTimestampsRing>,
    start: SystemTime,
    prev_period: AtomicU32,
}

impl Tracer {
    pub fn new() -> Self {
        let now = SystemTime::now();

        let ringbuf = Arc::new(AtomicTimestampsRing::new());

        let handle = recv_thread(ringbuf.clone());
        Self {
            ringbuf,
            handle: Some(handle),
            start: now,
            prev_period: AtomicU32::new(0),
        }
    }
    pub fn event(&self, v: u8) {
        let mut first = true;
        let now = self.start.elapsed().unwrap().as_nanos();
        let now_pr = (now >> 8) as u32;
        let prev_pr = self.prev_period.swap(now_pr, Ordering::Relaxed);
        let mut dif = now_pr.saturating_sub(prev_pr);
        let now = now as u8;
        /// Write event packet
        let _ = self.ringbuf.try_push_with(2, |_, _| {
            if first {
                first = false;
                v | 0x80 // first bit is 1 - event packet id
            }
            else {
                now
            }
        });
        while dif > 0 {
            let cur_dif = dif.min(0x7F) as u8;
            let _ = self.ringbuf.try_push(cur_dif);
            dif -= cur_dif as u32;
        }
    }
}

impl Drop for Tracer {
    fn drop(&mut self) {
        let _ = mem::replace(&mut self.ringbuf, Arc::new(AtomicTimestampsRing::new()));
        self.handle.take().unwrap().join().unwrap()
    }
}