use std::arch::x86_64::_rdtsc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

#[inline(always)]
pub fn now() -> u64 {
    // unsafe { _rdtsc() }
    UNIX_EPOCH.elapsed().unwrap().as_nanos() as u64
}

static PREV_PERIOD: AtomicU64 = AtomicU64::new(0);

pub fn capture_timestamp() -> (u64, u16) {
    let mut prev_pr = PREV_PERIOD.load(Ordering::Relaxed);
    unsafe {
        loop {
            let now = now();
            // let now = 8234721;
            let now_pr = (now >> 16) as u64;
            let dif_pr = now_pr.unchecked_sub(prev_pr);
            if dif_pr > 0 {
                match PREV_PERIOD.compare_exchange(prev_pr, now_pr, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => {
                        return (dif_pr, now as u16);
                    },
                    Err(x) => prev_pr = x
                }
            } else {
                return (0, now as u16);
            }
        }
    }
}