use core::arch::asm;
use crate::timestamp::TimestampProvider;

pub struct AArch64Timestamp;

impl TimestampProvider for AArch64Timestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        let value: u64;
        unsafe {
            asm!("mrs {}, cntvct_el0", out(reg) value); // Read the virtual counter
        }
        value
    }
}