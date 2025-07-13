use crate::TimestampProvider;
use core::arch::asm;

pub struct EspIdfTimestamp;

impl TimestampProvider for EspIdfTimestamp {
    type TimestampType = u32;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        let cycles: u32;
        unsafe {
            asm!(
            "csrr {0}, 0x802",  // Works from any privilege level
            out(reg) cycles,
            );
        }
        cycles
    }
}