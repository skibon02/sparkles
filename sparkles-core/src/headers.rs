use alloc::string::String;
use serde::{Deserialize, Serialize};
use crate::local_storage::id_mapping::IdStoreMap;

/// This header describe byte chunk of sparkles' events
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LocalPacketHeader {
    pub thread_ord_id: u64,
    pub thread_info: Option<ThreadInfo>,

    pub start_timestamp: u64,
    pub end_timestamp: u64,

    pub id_store: IdStoreMap,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ThreadInfo {
    pub thread_id: u64,
    pub new_thread_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SparklesEncoderInfo {
    pub ver: u32,
    pub counts_per_ns: f64,
}