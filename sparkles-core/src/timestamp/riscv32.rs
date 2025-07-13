use core::arch::asm;
use crate::TimestampProvider;

pub struct RiscV32Timestamp;

impl TimestampProvider for RiscV32Timestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        unsafe {
            let mut cycles_lo: u32;
            let mut cycles_hi: u32;

            // Read the lower 32 bits of the cycle counter
            asm!("rdcycle {0}", out(reg) cycles_lo);
            // Read the upper 32 bits of the cycle counter (if available)
            asm!("rdcycleh {0}", out(reg) cycles_hi);

            // Combine into a 64-bit value
            ((cycles_hi as u64) << 32) | (cycles_lo as u64)
        }
    }
}