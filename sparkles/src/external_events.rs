use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use log::warn;
use parking_lot::Mutex;
use sparkles_core::protocol::packets::{ExternalEventNames, ExternalEvents};
use sparkles_core::StaticNameRepr;

static LAST_EXT_ORD_ID: AtomicU32 = AtomicU32::new(0);

/// Helper for recording external events.
/// External events is an abstraction for any source of events bound to potentially different time domain.
/// This time domain is synchronized with the main system time domain (used by std::time::Instant).
pub struct ExternalEventsSource {
    name: Arc<str>,
    ext_ord_id: u32,
    event_names: HashMap<u32, (&'static str, u16)>,
    prev_events_len: usize,
}

impl ExternalEventsSource {
    /// Create a new `ExternalEventsRecorder` with the given name.
    pub fn new(name: String) -> Self {
        let ext_ord_id = LAST_EXT_ORD_ID.fetch_add(1, Ordering::Relaxed);
        ExternalEventsSource {
            name: name.into(),
            ext_ord_id,
            event_names: HashMap::new(),
            prev_events_len: 0,
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
        if local_timestamp == 0 || external_timestamp == 0 {
            warn!("Timestamps must be non-zero in ExternalEventsSource '{}'. Ignoring sync point.", self.name);
            return;
        }

        EXTERNAL_EVENTS_SYNC_POINTS.lock().push((self.ext_ord_id, local_timestamp, external_timestamp));
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

    /// event_names: slice of (event_name_id, pairing_id_flag) tuples.
    /// pairing_id: pairing_id_flag (1-127) ( | 0x80 for range end ). 0 for Instant event. Value 128 is invalid.
    pub fn push_events(&mut self, timestamps: &[u64], event_names: &[(u16, u8)]) {
        if timestamps.len() != event_names.len() {
            warn!("timestamps.len() must be equal to event_names.len() in ExternalEventsSource '{}'. Ignoring events.", self.name);
            return;
        }

        if self.event_names.len() != self.prev_events_len {
            self.prev_events_len = self.event_names.len();

            let mut event_names = vec![String::new(); self.event_names.len()];
            for (name, ord_id) in self.event_names.values() {
                event_names[*ord_id as usize] = name.to_string();
            }

            // Send event names
            let event_names = ExternalEventNames {
                ext_ord_id: self.ext_ord_id,
                channel_name: self.name.clone(),
                event_names,
            };
            EXTERNAL_EVENTS_NAMES.lock().push(event_names);
        }

        let min_tm = timestamps.iter().min().unwrap_or(&0);
        let max_tm = timestamps.iter().max().unwrap_or(&0);
        let bytes_per_tm = ((max_tm - min_tm).next_power_of_two().trailing_zeros() as usize).div_ceil(8);

        let mut buf = Vec::with_capacity(timestamps.len() * (3 + bytes_per_tm));
        for (timestamp, (ev_name, ev_pairing_id)) in timestamps.iter().zip(event_names.iter()) {
            if *ev_pairing_id == 128 {
                panic!("pairing_id 128 is invalid in ExternalEventsSource '{}'. Aborting.", self.name);
            }
            let tm = (*timestamp - min_tm).to_be_bytes();
            let ev_id = ev_name.to_be_bytes();
            buf.extend_from_slice(&tm[..bytes_per_tm]);
            buf.extend_from_slice(&ev_id);
            buf.push(*ev_pairing_id);
        }

        let header = ExternalEvents {
            ext_ord_id: self.ext_ord_id,
            start_timestamp: *min_tm,
            bytes_per_timestamp: bytes_per_tm as u8,
        };
        EXTERNAL_EVENTS_PACKETS.lock().push((header, buf));
    }
}

pub(crate) static EXTERNAL_EVENTS_PACKETS: Mutex<Vec<(ExternalEvents, Vec<u8>)>> = Mutex::new(Vec::new());
pub(crate) static EXTERNAL_EVENTS_SYNC_POINTS: Mutex<Vec<(u32, u64, u64)>> = Mutex::new(Vec::new());
pub(crate) static EXTERNAL_EVENTS_NAMES: Mutex<Vec<ExternalEventNames>> = Mutex::new(Vec::new());