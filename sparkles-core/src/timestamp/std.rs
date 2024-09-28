extern crate std;

use std::time::UNIX_EPOCH;
use crate::TimestampProvider;

pub struct StdTimestamp;

// TODO: consider increasing TimestampType to avoid overflow
impl TimestampProvider for StdTimestamp {
    type TimestampType = u64;

    #[inline(always)]
    fn now() -> Self::TimestampType {
        UNIX_EPOCH.elapsed().unwrap().as_nanos() as u64
    }
}