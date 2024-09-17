//! Timestamps adaptively choose implementation depending on architecture, std support and `cortex-m` feature
//!
//! Priority order:
//! 1. If your CPU architecture is x86, `X86Timestamp` is used
//! 2. Otherwise, if you're in std environment, `std::time::Instant` is selected as timestamp provider.
//! 3. If feature `cortex-m` is active, `CortexMTimestamp` is used.
//! 4. If none of above is true, compile error is emitted.

#[cfg(any(target_arch="x86", target_arch="x86_64"))]
pub mod x86;
#[cfg(any(target_arch="x86", target_arch="x86_64"))]
pub use x86::X86Timestamp as Timestamp;

#[cfg(all(not(target_os="none"), not(any(target_arch="x86", target_arch="x86_64"))))]
pub mod std;
#[cfg(all(not(target_os="none"), not(any(target_arch="x86", target_arch="x86_64"))))]
pub use std::StdTimestamp as Timestamp;

#[cfg(feature="cortex-m")]
pub mod cortex_m;
#[cfg(feature="cortex-m")]
pub use cortex_m::CortexMTimestamp as Timestamp;

#[cfg(not(any(target_arch="x86", target_arch="x86_64", feature="cortex-m", not(target_os="none"))))]
compile_error!("Unsupported platform! Either std or cortex-m are currently supported");

/// TimestampProvider is a source for relatively stable timestamp, which wraps around after reaching maximum value.
///
/// Maximum value is defined as unsigned integer composed of TIMESTAMP_VALID_BITS binary ones.
pub trait TimestampProvider {
    /// Numeric timestamp type, can be either u32 or u64.
    type TimestampType: Copy + Sized;

    /// Returns current timestamp from provider.
    fn now() -> Self::TimestampType;

    /// Define how many bits are valid in timestamp, returned from now()
    const TIMESTAMP_VALID_BITS: u16 = (size_of::<Self::TimestampType>() as u16) << 3;
    
    const COUNTS_PER_NS: f64;
}