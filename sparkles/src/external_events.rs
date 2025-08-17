use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use log::warn;
use sparkles_core::StaticNameRepr;

static LAST_EXT_ORD_ID: AtomicU32 = AtomicU32::new(0);

/// Helper for recording external events.
/// External events is an abstraction for any source of events bound to potentially different time domain.
/// This time domain is synchronized with the main system time domain (used by std::time::Instant).
pub struct ExternalEventsSource {
    name: String,
    ext_ord_id: u32,
    event_names: HashMap<u32, (&'static str, u16)>,
}

impl ExternalEventsSource {
    /// Create a new `ExternalEventsRecorder` with the given name.
    pub fn new(name: String) -> Self {
        let ext_ord_id = LAST_EXT_ORD_ID.fetch_add(1, Ordering::Relaxed);
        ExternalEventsSource {
            name,
            ext_ord_id,
            event_names: HashMap::new(),
        }
    }

    /// Push new synchronization point of local and external timestamps, captured at the same time.
    ///
    /// Local timestamp: The one returned by `sparkles::monotonic::get_monotonic()`.
    /// External timestamp: The one used in events from this external source.
    ///
    /// Note! You need at least two calls to `push_sync_point` before adding new events. Otherwise, events will be ignored.
    /// Recommended delay between first two calls: >=1ms.
    ///
    /// You can periodically call this function to add new synchronization points as time progresses.
    /// This will compensate any drift between two timestamp sources by the parser.
    pub fn push_sync_point(&mut self, local_timestamp: u64, external_timestamp: u64) {

    }


    /// Get u16 value representing certain event name.
    /// If this name was not registered before, it will be registered and assigned a new ordinal ID.
    pub fn encode_event_name(&mut self, name: StaticNameRepr) -> u16 {
        if let Some((_, ord_id)) = self.event_names.get(&name.hash()) {
            *ord_id
        } else {
            let ord_id = self.event_names.len() as u16;
            if ord_id == u16::MAX {
                warn!("Too many event names registered in ExternalEventsSource '{}'. Maximum is 65535.", self.name);
                u16::MAX
            }
            else {
                self.event_names.insert(name.hash(), (name.string(), ord_id));
                ord_id
            }
        }
    }

    pub fn push_events(&mut self, timestamps: &[u64], event_names: &[u16]) {
        if timestamps.len() != event_names.len() {
            warn!("Timestamps and event names arrays have different lengths in ExternalEventsSource '{}'.", self.name);
            return;
        }

        let mut buf = Vec::with_capacity(timestamps.len() * 6);
        for (timestamp, event_name) in timestamps.iter().zip(event_names.iter()) {
            let tm = timestamp.to_be_bytes();
            let ev_id = event_name.to_be_bytes();
            buf.extend_from_slice(&tm);
            buf.extend_from_slice(&ev_id);
        }
    }
}