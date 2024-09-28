#[cfg(target_arch="x86")]
use core::arch::x86::{__rdtscp, _rdtsc};
#[cfg(target_arch="x86_64")]
use core::arch::x86_64::{__rdtscp, _rdtsc};
use crate::timestamp::TimestampProvider;

pub struct X86Timestamp;

impl TimestampProvider for X86Timestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        unsafe {
            #[cfg(feature = "accurate-timestamps-x86")]
            let v = __rdtscp(&mut 0);
            #[cfg(not(feature = "accurate-timestamps-x86"))]
            let v = _rdtsc();

            v
        }
    }
}