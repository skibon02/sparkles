use std::time::UNIX_EPOCH;
use crate::TimestampProvider;

pub struct InstantTimestamp;

// TODO: consider increasing TimestampType to avoid overflow
impl TimestampProvider for InstantTimestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        UNIX_EPOCH.elapsed().unwrap().as_nanos() as u64
    }

    const COUNTS_PER_NS: f64 = 1.0;
}