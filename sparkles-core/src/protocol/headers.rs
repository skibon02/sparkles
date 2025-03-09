use alloc::string::{String, ToString};
use bincode::{Decode, Encode};
use crate::local_storage::id_mapping::IdMapping;
use crate::{Timestamp, TimestampProvider};
use crate::consts::PROTOCOL_VERSION;

/// This header describe byte buffer filled with encoded sparkles events.
/// This header is thread-local. Each thread events packet has its own header and buffer.
#[derive(Encode, Decode, Clone, Debug, Default)]
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

#[derive(Encode, Decode, Clone, Debug, Default)]
pub struct ThreadInfo {
    pub thread_id: u64,
    pub new_thread_name: Option<String>,
}

#[derive(Encode, Decode, Clone, Debug)]
pub struct SparklesMachineInfo {
    pub ver: (u8, u8),
    pub process_name: String,
    pub pid: u32,
    pub timestamp_max_value: u64
}

impl SparklesMachineInfo {
    pub fn new(process_name: String, pid: u32) -> Self {
        Self {
            pid,
            process_name,
            ver: PROTOCOL_VERSION,
            timestamp_max_value: Timestamp::MAX_VALUE,
        }
    }
}

impl Default for SparklesMachineInfo {
    fn default() -> Self {
        Self {
            process_name: "unknown".to_string(),
            pid: 0,
            ver: PROTOCOL_VERSION,
            timestamp_max_value: Timestamp::MAX_VALUE,
        }
    }
}