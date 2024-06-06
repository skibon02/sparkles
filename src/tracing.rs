use std::{mem, thread};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
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
}

impl Tracer {
    pub fn new() -> Self {

        let ringbuf = Arc::new(AtomicTimestampsRing::new());

        let handle = recv_thread(ringbuf.clone());
        Self {
            ringbuf,
            handle: Some(handle)
        }
    }
    pub fn event(&self, v: u8) {
        while self.ringbuf.try_push(v).is_none() {}
    }
}

impl Drop for Tracer {
    fn drop(&mut self) {
        let _ = mem::replace(&mut self.ringbuf, Arc::new(AtomicTimestampsRing::new()));
        self.handle.take().unwrap().join().unwrap()
    }
}