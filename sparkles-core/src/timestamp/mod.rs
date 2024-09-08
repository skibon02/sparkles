#[cfg(any(target_arch="x86", target_arch="x86_64"))]
pub mod x86;

#[cfg(any(target_arch="x86", target_arch="x86_64"))]
pub use x86::X86Timestamp as Timestamp;

pub mod instant;
#[cfg(not(any(target_arch="x86", target_arch="x86_64")))]
pub use instant::InstantTimestamp as Timestamp;

/// TimestampProvider is a source for relatively stable timestamp, which wraps around after reaching maximum value.
///
/// Maximum value is defined as unsigned integer composed of TIMESTAMP_VALID_BYTES binary ones.
pub trait TimestampProvider {
    type TimestampType: Copy + Sized;

    /// Returns current timestamp from provider.
    fn now() -> Self::TimestampType;

    /// Define how many bits are valid in timestamp, returned from now()
    const TIMESTAMP_VALID_BITS: u16 = (size_of::<Self::TimestampType>() as u16) << 3;
    
    const COUNTS_PER_NS: f64;
}