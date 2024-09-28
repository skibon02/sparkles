use alloc::string::{String, ToString};
use serde::{Deserialize, Serialize};
use crate::local_storage::id_mapping::IdMapping;
use crate::{Timestamp, TimestampProvider};

/// This header describe byte buffer filled with encoded sparkles events.
/// This header is thread-local. Each thread events packet has its own header and buffer.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LocalPacketHeader {
    /// Globally unique order number of the spawned thread
    pub thread_ord_id: u64,
    pub thread_info: Option<ThreadInfo>,

    /// Timestamp of the first event in a buffer
    pub start_timestamp: u64,
    /// Timestamp of the last event in a buffer
    pub end_timestamp: u64,

    pub id_store: IdMapping,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ThreadInfo {
    pub thread_id: u64,
    pub new_thread_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SparklesEncoderInfo {
    pub ver: u32,
    pub process_name: String,
    pub pid: u32,
    pub timestamp_max_value: u64
}

impl SparklesEncoderInfo {
    pub fn new(process_name: String, pid: u32) -> Self {
        Self {
            pid,
            process_name,
            ver: crate::consts::ENCODER_VERSION,
            timestamp_max_value: Timestamp::MAX_VALUE,
        }
    }
}

impl Default for SparklesEncoderInfo {
    fn default() -> Self {
        Self {
            process_name: "unknown".to_string(),
            pid: 0,
            ver: crate::consts::ENCODER_VERSION,
            timestamp_max_value: Timestamp::MAX_VALUE,
        }
    }
}