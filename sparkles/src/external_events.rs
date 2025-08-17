use std::sync::atomic::{AtomicU32, Ordering};

static LAST_EXT_ORD_ID: AtomicU32 = AtomicU32::new(0);

/// Helper for recording external events.
/// External events is an abstraction for any source of events bound to potentially different time domain.
/// This time domain is synchronized with the main system time domain (used by std::time::Instant).
pub struct ExternalEventsRecorder {
    name: String,
    ext_ord_id: u32,
}

impl ExternalEventsRecorder {
    /// Create a new `ExternalEventsRecorder` with the given name.
    pub fn new(name: String) -> Self {
        let ext_ord_id = LAST_EXT_ORD_ID.fetch_add(1, Ordering::Relaxed);
        ExternalEventsRecorder {
            name,
            ext_ord_id,
        }
    }

    pub fn push_sync_points(&mut self, local_timestamp: std::time::Instant, external_timestamp: u64) {

    }
}