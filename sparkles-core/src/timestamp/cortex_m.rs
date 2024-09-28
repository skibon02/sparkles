use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m::peripheral::DWT;
use crate::TimestampProvider;


static IS_INIT: AtomicBool = AtomicBool::new(false);
pub fn init() {
    let mut cp = unsafe { cortex_m::Peripherals::steal() };

    cp.DCB.enable_trace();
    DWT::unlock();
    cp.DWT.enable_cycle_counter();
    IS_INIT.store(true, Ordering::Relaxed);
}

pub struct CortexMTimestamp;

impl TimestampProvider for CortexMTimestamp {
    type TimestampType = u32;

    /// Get current cortex-m cyccnt value.
    ///
    /// # Panic
    /// Panics if `init()` was not called first.
    #[inline(always)]
    fn now() -> Self::TimestampType {
        if !IS_INIT.load(Ordering::Relaxed) {
            panic!("Attempt to get cyccnt without initialization! Must call init() first");
        }
        unsafe { (&*DWT::PTR).cyccnt.read() }
    }
}