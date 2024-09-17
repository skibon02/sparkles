use alloc::string::String;
use serde::{Deserialize, Serialize};
use crate::local_storage::id_mapping::IdStoreMap;

/// This header describe byte chunk of sparkles' events
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LocalPacketHeader {
    pub thread_ord_id: u64,
    pub thread_id: u64,

    pub start_timestamp: u64,
    pub end_timestamp: u64,

    pub id_store: IdStoreMap,

    // Keep it here to make parser simpler
    pub counts_per_ns: f64,
}

/// This header describe updated thread name
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThreadNameHeader {
    pub thread_ord_id: u64,
    pub thread_name: String,
}