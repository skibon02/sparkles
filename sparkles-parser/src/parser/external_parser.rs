mod raw_decoder;

use std::collections::BTreeMap;
use std::iter;
use std::ops::Deref;
use std::rc::Rc;
use auto_enums::auto_enum;
use indexmap::IndexMap;
use log::{error, warn};
use sparkles_core::protocol::packets::{ExternalEventNames, ExternalEvents};
use crate::{TimeSyncPoints, TracingEventId};
use crate::parsed::{ExternalChannelInfo, ParsedExternalEvent};
use crate::parser::external_parser::raw_decoder::{decode_raw_event, RawForeignTracingEvent};

pub struct ExternalParserState {
    ext_ord_id: u32,
    channel_name: Option<Rc<str>>,
    id_store: IndexMap<TracingEventId, Rc<str>>,

    started_ranges: BTreeMap<u8, (TracingEventId, u64)>,

    time_sync_points: TimeSyncPoints,
}

pub enum ExternalParserEvent {
    NewEvents(Vec<ParsedExternalEvent>),
    NewEventNames(IndexMap<TracingEventId, Rc<str>>),
}

impl ExternalParserState {
    pub fn new(ext_ord_id: u32) -> Self {
        Self {
            ext_ord_id,
            channel_name: None,
            id_store: IndexMap::new(),
            started_ranges: BTreeMap::new(),
            time_sync_points: TimeSyncPoints::new(),
        }
    }
    pub fn add_time_sync_point(&mut self, external_tm: u64, local_tm: u64) {
        self.time_sync_points.add_time_sync_point(external_tm, local_tm);
    }

    #[must_use]
    #[auto_enum(Iterator)]
    pub fn got_events(&mut self, header: ExternalEvents, events: &Vec<u8>) -> impl Iterator<Item=ExternalParserEvent> {
        if self.time_sync_points.is_empty() {
            error!("ExternalEvents packet received before ExternalSyncPoint! Dropping events...");
            return iter::empty();
        }

        let mut parsed = Vec::with_capacity(events.len() / (2 + header.bytes_per_timestamp as usize));
        for event_bytes in events.chunks(2 + header.bytes_per_timestamp as usize) {
            if event_bytes.len() < 2 + header.bytes_per_timestamp as usize {
                warn!("ExternalEvents packet has incomplete event! Skipping...");
                continue;
            }

            if let Some(ev) = decode_raw_event(event_bytes, &header) {
                self.handle_raw_event(ev, &mut parsed);
            }
        }

        if !parsed.is_empty() {
            iter::once(ExternalParserEvent::NewEvents(parsed))
        }
        else {
            iter::empty()
        }
    }

    fn handle_raw_event(&mut self, ev: RawForeignTracingEvent, parsed: &mut Vec<ParsedExternalEvent>) -> Option<ParsedExternalEvent> {
        match ev {
            RawForeignTracingEvent::Instant {
                name_id,
                raw_tm,
            } => {
                let interpolated_tm = self.time_sync_points.project_tm(raw_tm);
                let Some(tm) = interpolated_tm else {
                    warn!("Not enough time sync points! Dropping external instant event...");
                    return None;
                };

                Some(ParsedExternalEvent::Instant{
                    name_id,
                    tm
                })
            }
            RawForeignTracingEvent::RangePart {
                pairing_id,
                name_id: ev_id,
                raw_tm,
                is_end
            } => {
                let interpolated_tm = self.time_sync_points.project_tm(raw_tm);
                if is_end {
                    let Some(tm) = interpolated_tm else {
                        warn!("Not enough time sync points! Dropping external range event end...");
                        return None;
                    };

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
                    let Some(tm) = interpolated_tm else {
                        warn!("Not enough time sync points! Dropping external range event start...");
                        return None;
                    };
                    // New range start
                    self.started_ranges.insert(pairing_id, (ev_id, tm));
                    None
                }
            }
        }
    }

    #[must_use]
    #[auto_enum(Iterator)]
    pub fn got_event_names(&mut self, names: ExternalEventNames) -> impl Iterator<Item=ExternalParserEvent> {

        // Update id store
        let mut something_changed = false;
        for (id, name) in names.event_names.iter().enumerate() {
            let id = id as u8;
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
