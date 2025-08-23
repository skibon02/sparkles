use std::collections::BTreeMap;
use std::iter;
use std::ops::Deref;
use std::rc::Rc;
use auto_enums::auto_enum;
use indexmap::IndexMap;
use log::{error, warn};
use sparkles_core::protocol::packets::{ExternalEventNames, ExternalEvents};
use crate::{InterpolationPoints, TracingEventId, TracingStats};
use crate::parsed::ParsedExternalEvent;

#[derive(Default)]
pub struct ExternalParserState {
    channel_name: Option<Rc<str>>,
    id_store: IndexMap<TracingEventId, Rc<str>>,

    started_ranges: BTreeMap<u8, (TracingEventId, u64)>,

    interpolation_points: InterpolationPoints,
}

pub enum ExternalParserEvent {
    NewEvents(Vec<ParsedExternalEvent>),
    NewEventNames(IndexMap<TracingEventId, Rc<str>>),
}

impl ExternalParserState {
    pub fn add_interpolation_point(&mut self, p0: u64, p1: u64) {
        self.interpolation_points.add_interpolation_point(p0, p1);
    }

    #[must_use]
    #[auto_enum(Iterator)]
    pub fn got_events(&mut self, header: ExternalEvents, events: &Vec<u8>) -> impl Iterator<Item=ExternalParserEvent> {
        let start_tm = header.start_timestamp;

        if self.interpolation_points.is_empty() {
            error!("ExternalEvents packet received before ExternalSyncPoint! Dropping events...");
            return iter::empty();
        }

        let mut parsed = Vec::with_capacity(events.len() / (2 + header.bytes_per_timestamp as usize));
        for event_bytes in events.chunks(2 + header.bytes_per_timestamp as usize) {
            if event_bytes.len() < 2 + header.bytes_per_timestamp as usize {
                warn!("ExternalEvents packet has incomplete event! Skipping...");
                continue;
            }
            let mut tm_bytes = [0u8; 8];
            tm_bytes[8 - header.bytes_per_timestamp as usize..].copy_from_slice(&event_bytes[..header.bytes_per_timestamp as usize]);
            let tm = start_tm + u64::from_be_bytes(tm_bytes);
            let ev_id = event_bytes[header.bytes_per_timestamp as usize];
            let pairing_id = event_bytes[header.bytes_per_timestamp as usize + 1];

            if pairing_id == 0 {
                parsed.push(ParsedExternalEvent::Instant{
                    name_id: ev_id,
                    tm: self.interpolation_points.project_tm(tm)
                })
            }
            else {
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
                        start: self.interpolation_points.project_tm(start_tm),
                        end: self.interpolation_points.project_tm(tm),
                    };
                    parsed.push(parsed_event);
                }
                else {
                    // New range start
                    self.started_ranges.insert(pairing_id, (ev_id, tm));
                }
            }
        }

        if !parsed.is_empty() {
            iter::once(ExternalParserEvent::NewEvents(parsed))
        }
        else {
            iter::empty()
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
}
