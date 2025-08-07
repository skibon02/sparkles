use core::arch::asm;
use crate::TimestampProvider;

pub struct XtensaEsp32Timestamp;

impl TimestampProvider for XtensaEsp32Timestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        unsafe {
            let ccount: u32;

            // Read the CCOUNT register (32-bit cycle counter)
            asm!("rsr {0}, ccount", out(reg) ccount);

            // Extend to 64-bit for consistency with RISC-V implementation
            ccount as u64
        }
    }
}