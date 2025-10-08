
/// Kind of monotonic time source used.
pub enum MonotonicSource {
    /// Corresponds to `clock_gettime(CLOCK_MONOTONIC)` on Linux and Android systems.
    /// Timestamp is in nanoseconds.
    ClockMonotonic,
    /// Corresponds to `QueryPerformanceCounter` on Windows systems.
    /// Note! Returned value is not in nanoseconds. Only `counter.QuadPart()` is returned
    PerformanceCounter,
    /// Fallback option that uses `Instant::now()`.
    /// Represents the current time since the program started (or first internal use of monotonic timestamp).
    /// Timestamp is in nanoseconds.
    Instant
}

/// Get the monotonic time source used by the current platform.
///
/// This can be relevant if you are using external timestamps source with time domain synchronization mechanism with host time domain.
/// Example: Timestamp queries in Vulkan API and VK_EXT_calibrated_timestamps extension.
pub const fn monotonic_source() -> MonotonicSource {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        MonotonicSource::ClockMonotonic
    }
    #[cfg(target_os = "windows")]
    {
        MonotonicSource::PerformanceCounter
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "windows")))]
    {
        MonotonicSource::Instant
    }
}

use std::sync::OnceLock;
use std::time::Instant;
static START_TM: OnceLock<Instant> = OnceLock::new();
pub(crate) fn get_start_tm() -> Instant {
    *START_TM.get_or_init(Instant::now)
}


/// Get the current monotonic time in nanoseconds.
/// This is a platform-specific implementation for retrieving a monotonic clock value.
/// This clock is not used directly for timestamps, but rather for adjusting and giving stability to local and fast CPU timestamp
///
/// Any external events if used should be synchronized with this clock. Use `monotonic_source()` to get the source of monotonic time.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn get_monotonic() -> u64 {
    use libc::{clock_gettime, timespec, CLOCK_MONOTONIC};

    let mut ts = timespec { tv_sec: 0, tv_nsec: 0 };
    let result = unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) };

    if result == 0 {
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }
    else {
        fallback_get_monotonic()
    }
}
#[cfg(target_os = "windows")]
pub fn get_monotonic() -> u64 {
    use winapi::um::profileapi::QueryPerformanceCounter;
    use winapi::shared::ntdef::LARGE_INTEGER;

    unsafe {
        let mut counter: LARGE_INTEGER = std::mem::zeroed();

        if QueryPerformanceCounter(&mut counter) != 0 {
            *counter.QuadPart() as u64
        } else {
            fallback_get_monotonic()
        }
    }
}

/// Get the current monotonic time in nanoseconds.
///
/// This function normalizes the platform-specific monotonic time to nanoseconds:
/// - Linux/Android: Returns nanoseconds since boot (same as `get_monotonic()`)
/// - Windows: Converts QueryPerformanceCounter to nanoseconds using QueryPerformanceFrequency
/// - Other platforms: Returns nanoseconds since program start
///
/// Use this when you need consistent nanosecond precision across platforms.
pub fn get_monotonic_nanos() -> u64 {
    match monotonic_source() {
        MonotonicSource::ClockMonotonic => {
            get_monotonic()
        }
        MonotonicSource::Instant => {
            get_monotonic()
        }
        MonotonicSource::PerformanceCounter => {
            #[cfg(target_os = "windows")]
            {
                let counter = get_monotonic();
                let frequency = get_perf_frequency_windows();

                counter * 1_000_000_000 / frequency
            }
            #[cfg(not(target_os = "windows"))]
            {
                unreachable!();
            }
        }
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "windows"
)))]
pub fn get_monotonic() -> u64 {
    fallback_get_monotonic()
}

fn fallback_get_monotonic() -> u64 {
    // Fallback to system time if the primary method fails
    Instant::now()
        .duration_since(get_start_tm())
        .as_nanos()
        .min(u64::MAX as u128) as u64
}

// Windows-specific: Cache performance frequency for nanosecond conversion
#[cfg(target_os = "windows")]
static PERF_FREQUENCY: OnceLock<u64> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn get_perf_frequency_windows() -> u64 {
    *PERF_FREQUENCY.get_or_init(|| {
        use winapi::um::profileapi::QueryPerformanceFrequency;
        use winapi::shared::ntdef::LARGE_INTEGER;

        unsafe {
            let mut freq: LARGE_INTEGER = std::mem::zeroed();
            if QueryPerformanceFrequency(&mut freq) != 0 {
                *freq.QuadPart() as u64
            } else {
                1_000_000_000 // Fallback to 1 GHz if query fails
            }
        }
    })
}
