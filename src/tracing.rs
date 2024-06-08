use std::{mem, thread};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::SystemTime;
use log::{info, warn};
use crate::fifo::AtomicTimestampsRing;

pub static IS_FINISHED: AtomicBool = AtomicBool::new(false);

fn recv_thread(rx: Arc<AtomicTimestampsRing>) -> JoinHandle<()> {
    let mut accum: u64 = 0;
    thread::spawn(move || {
        while !IS_FINISHED.load(Ordering::Relaxed) {
            let bytes = rx.try_pop::<50>();
            if bytes.is_some() {
                // if accum == 0 {
                //     let bytes = bytes.unwrap();
                //     let mut metrics = BTreeMap::new();
                //     // go over 1
                //     for i in (0..50).step_by(2) {
                //         let is_event_packet = bytes[i] & 0x80 != 0;
                //         if is_event_packet {
                //             let event_id = bytes[i] & 0x7F;
                //             let timestamp = bytes[i + 1];
                //             metrics.insert(i, (timestamp, 0));
                //         }
                //         else {
                //             let pr = bytes[i];
                //             let ind_diff = bytes[i + 1] as i8;
                //             match metrics.entry(i + 1 - ind_diff as usize) {
                //                 std::collections::btree_map::Entry::Occupied(mut entry) => {
                //                     let (_, accum_pr) = entry.get_mut();
                //                     *accum_pr += pr as u32;
                //                 },
                //                 std::collections::btree_map::Entry::Vacant(_) => {
                //                     warn!("Missing event packet for index {}", i);
                //                 }
                //
                //             }
                //         }
                //     }
                //
                //     for (i, (timestamp, accum_pr)) in metrics {
                //         info!("Event packet: {} at timestamp: {} with accum_pr: {}", i, timestamp, accum_pr);
                //     }
                // }
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

    // 24 bits are significant
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
        let (mut dif_pr, now) = self.capture_timestamp();
        /// Write event packet
        let mut first = true;
        let mut ind1 = 0;
        let _ = self.ringbuf.try_push_with(2, |index| {
            if first {
                first = false;

                ind1 = index;
                v | 0x80 // first bit is 1 - event packet id
            }
            else {
                now
            }
        });
        // While value is 32 bits, we send 7 bits at a time
        while dif_pr > 0 {
            let cur_dif = dif_pr.min(0x7F) as u8;
            let mut first = true;
            let _ = self.ringbuf.try_push_with(2, |ind2| {
                if first {
                    first = false;

                    cur_dif
                }
                else {
                    let ind_diff = (ind2 - ind1) as i8;
                    ind_diff as u8
                }
            });
            dif_pr -= cur_dif as u32;
        }
    }

    /// returns dif_pr and 8 last bits of timestamp
    fn capture_timestamp(&self) -> (u32, u8) {
        let mut prev_pr = self.prev_period.load(Ordering::Relaxed);
        loop {
            let now = self.start.elapsed().unwrap().as_nanos() as u64;// 36ns
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
    }
}

impl Drop for Tracer {
    fn drop(&mut self) {
        let _ = mem::replace(&mut self.ringbuf, Arc::new(AtomicTimestampsRing::new()));
        self.handle.take().unwrap().join().unwrap()
    }
}