mod raw_decoder;

use std::collections::{BTreeMap, VecDeque};
use std::iter;
use std::mem::take;
use std::ops::Deref;
use std::rc::Rc;
use auto_enums::auto_enum;
use indexmap::IndexMap;
use log::{error, warn};
use sparkles_core::protocol::packets::{ExternalEventNames, ExternalEvents};
use crate::{TimeSyncPoints, ExternalEventNameId};
use crate::parsed::{ExternalChannelInfo, ParsedExternalEvent};
use crate::parser::external_parser::raw_decoder::{decode_raw_event, RawExternalTracingEvent};

pub type ExternalEventNamesStore = IndexMap<ExternalEventNameId, Rc<str>>;

pub struct ExternalParserState {
    ext_ord_id: u32,
    channel_name: Option<Rc<str>>,
    id_store: ExternalEventNamesStore,

    started_ranges: BTreeMap<u8, (ExternalEventNameId, u64)>,

    time_sync_points: TimeSyncPoints,
    unhandled_events: VecDeque<RawExternalTracingEvent>,
}

pub enum ExternalParserEvent {
    NewEvents(Vec<ParsedExternalEvent>),
    NewEventNames(ExternalEventNamesStore),
}

impl ExternalParserState {
    pub fn new(ext_ord_id: u32) -> Self {
        Self {
            ext_ord_id,
            channel_name: None,
            id_store: IndexMap::new(),
            started_ranges: BTreeMap::new(),
            time_sync_points: TimeSyncPoints::new(),
            unhandled_events: VecDeque::new(),
        }
    }
    pub fn add_time_sync_point(&mut self, local_tm: u64, external_tm: u64) {
        self.time_sync_points.add_time_sync_point(local_tm, external_tm);
    }

    #[auto_enum(Iterator)]
    pub fn got_events(&mut self, header: ExternalEvents, events: &[u8]) -> impl Iterator<Item=ExternalParserEvent> {
        if self.time_sync_points.is_empty() {
            error!("ExternalEvents packet received before ExternalSyncPoint! Dropping events...");
            return iter::empty();
        }

        // 1) decode raw events
        for event_bytes in events.chunks(3 + header.bytes_per_timestamp as usize) {
            if event_bytes.len() < 3 + header.bytes_per_timestamp as usize {
                warn!("ExternalEvents packet has incomplete event! Skipping...");
                continue;
            }

            if let Some(ev) = decode_raw_event(event_bytes, &header) {
                self.unhandled_events.push_back(ev);
            }
        }

        // 2) parse unhandled events
        if let Some(parsed) = self.parse_unhandled_events(false) {
            iter::once(ExternalParserEvent::NewEvents(parsed))
        }
        else {
            iter::empty()
        }
    }

    pub fn parse_unhandled_events(&mut self, is_final: bool) -> Option<Vec<ParsedExternalEvent>> {
        let (start, end) = self.time_sync_points.src_bounds()?;

        let processable_events = if is_final {
            Vec::from(take(&mut self.unhandled_events))
        }
        else {
            let len_to_handle = self.unhandled_events.partition_point(|ev| {ev.raw_timestamp() <= end});
            if len_to_handle == 0 {
                return None;
            }

            // make a split
            self.unhandled_events.drain(0..len_to_handle).collect()
        };


        let mut parsed = Vec::with_capacity(processable_events.len() / 2);
        for ev in processable_events {
            if let Some(parsed_ev) = self.handle_raw_event(ev, is_final) {
                parsed.push(parsed_ev);
            }
        }

        if !parsed.is_empty() {
            Some(parsed)
        }
        else {
            None
        }
    }

    fn handle_raw_event(&mut self, ev: RawExternalTracingEvent, is_final: bool) -> Option<ParsedExternalEvent> {
        let raw_tm = ev.raw_timestamp();
        let tm = if is_final {
            self.time_sync_points.project_tm_predict(raw_tm).unwrap()
        }
        else {
            let interpolated_tm = self.time_sync_points.project_tm(raw_tm);
            let Some(tm) = interpolated_tm else {
                warn!("Not enough time sync points! Dropping external event...");
                return None;
            };
            tm
        };
        match ev {
            RawExternalTracingEvent::Instant {
                name_id,
                raw_tm,
            } => {

                Some(ParsedExternalEvent::Instant{
                    name_id,
                    tm
                })
            }
            RawExternalTracingEvent::RangePart {
                pairing_id,
                name_id: ev_id,
                raw_tm,
                is_end
            } => {

                if is_end {
                    let start_event = self.started_ranges.remove(&pairing_id);
                    if let Some((start_ev_id, start_tm)) = start_event {
                        // Range end
                        let end_name_id = if ev_id == 0 {
                            None
                        }
                        else {
                            Some(ev_id)
                        };

                        let parsed_event = ParsedExternalEvent::Range {
                            name_id: start_ev_id,
                            end_name_id,
                            start: start_tm,
                            end: tm,
                        };
                        Some(parsed_event)
                    }
                    else {
                        // No matching start event
                        warn!("No matching start range part for external range event! ignoring...");
                        None
                    }
                }
                else {
                    // New range start
                    self.started_ranges.insert(pairing_id, (ev_id, tm));
                    None
                }
            }
        }
    }

    #[auto_enum(Iterator)]
    pub fn got_event_names(&mut self, names: ExternalEventNames) -> impl Iterator<Item=ExternalParserEvent> {

        // Update id store
        let mut something_changed = false;
        for (id, name) in names.event_names.iter().enumerate() {
            let id = id as ExternalEventNameId;
            if let Some(old_name) = self.id_store.get(&id) {
                if old_name.as_ref() != name.deref() {
                    something_changed = true;
                    error!("ID store mismatch for external channel {:?}#{:?}! ID: {}, Old: {:?}, New: {:?}", self.channel_name, id,
                                            id, old_name, name);
                }
            }
            else {
                something_changed = true;
            }
            self.id_store.insert(id, Rc::from(name.deref()));
        }

        // Update channel name
        self.channel_name = Some(names.channel_name.deref().into());

        if something_changed {
            iter::once(ExternalParserEvent::NewEventNames(self.id_store.clone()))
        }
        else {
            iter::empty()
        }
    }
    
    pub fn channel_info(&self) -> ExternalChannelInfo {
        ExternalChannelInfo {
            channel_name: self.channel_name.clone(),
            ext_ord_id: self.ext_ord_id,
        }
    }
}
