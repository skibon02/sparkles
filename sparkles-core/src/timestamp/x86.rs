#[cfg(target_arch="x86")]
use std::arch::x86::{__rdtscp, _rdtsc};
#[cfg(target_arch="x86_64")]
use std::arch::x86_64::{__rdtscp, _rdtsc};
use crate::timestamp::TimestampProvider;

pub struct X86Timestamp;

impl TimestampProvider for X86Timestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        unsafe {
            #[cfg(feature = "accurate-events-x86")]
            let v = __rdtscp(&mut 0);
            #[cfg(not(feature = "accurate-events-x86"))]
            let v = _rdtsc();

            v
        }
    }

    // TODO: May vary between CPUs
    const COUNTS_PER_NS: f64 = 2.495;
}