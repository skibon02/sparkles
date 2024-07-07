use std::arch::x86_64::{__rdtscp, _mm_lfence};

#[inline(always)]
pub fn now() -> u64 {
    unsafe {
        let mut aux: u32 = 0;
        // _mm_lfence();
        let v = __rdtscp(&mut aux as *mut u32);
        // _mm_lfence();

        v
    }
    // UNIX_EPOCH.elapsed().unwrap().as_nanos() as u64
}