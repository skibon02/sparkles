use std::collections::BTreeMap;
use std::mem;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Instant;
use indexmap::IndexMap;
use log::{debug, error, warn};
use sparkles_core::local_storage::id_mapping::EventType;
use sparkles_core::protocol::headers::LocalPacketHeader;
use crate::tracing_decoder::StreamFrameDecoder;
use crate::{ForeignRangeEnd, InterpolationPoints, TracingEventId, TracingStats};
use crate::parsed::{ParsedEvent, ThreadInfoState};

pub type EventNames = IndexMap<TracingEventId, (Rc<str>, EventType)>;

#[derive(Debug, Copy, Clone)]
pub enum TracingEvent {
    Instant(TracingEventId, u64),
    RangePart(TracingEventId, u64, u8),
    UnnamedRangeEnd(u64, u8),
    ForeignRangeEnd(Option<TracingEventId>, u64, u8, u64),
}

#[derive(Default)]
pub struct ThreadParserState {
    pub(crate) thread_name: Option<String>,
    thread_id: Option<u64>,
    last_thread_ord_id: u64,

    // start timestamp and duration for missed events packet
    missed_events: Vec<(u64, u64)>,

    // ---- TMP DATA ----
    state_machine: StreamFrameDecoder,
    // Helper for ranges handling
    cur_started_ranges: BTreeMap<u8, (TracingEventId, u64)>,
    // Storage for foreign range ends to be processed later
    foreign_range_ends: Vec<ForeignRangeEnd>,
    zero_diff_cnt: u64,

    id_store: EventNames,
    pub(crate) stats: TracingStats,
}

pub enum ThreadParserEvent {
    NewEvents(Vec<ParsedEvent>),
    EventNamesChanged(EventNames),
}
impl ThreadParserState {
    pub fn remove_foreign_range(&mut self, foreign_end_ord_id: u8) -> Option<(TracingEventId, u64)> {
        self.cur_started_ranges.remove(&foreign_end_ord_id)
    }

    pub fn take_foreign_range_ends(&mut self) -> Vec<ForeignRangeEnd> {
        mem::take(&mut self.foreign_range_ends)
    }
    pub fn store_foreign_ends(&mut self, foreign_range_ends: Vec<ForeignRangeEnd>) {
        self.foreign_range_ends = foreign_range_ends;
    }


    pub fn got_missed_events(&mut self, start: u64, dur: u64) {
        self.missed_events.push((start, dur));
    }

    #[must_use]
    pub fn got_events(&mut self, header: LocalPacketHeader, events_bytes: Vec<u8>, interpolation_points: &InterpolationPoints) -> Vec<ThreadParserEvent> {
        let thread_id = header.thread_ord_id;

        let mut res = vec![];

        //update thread name
        self.thread_id = Some(header.thread_info.thread_id);
        if let Some(thread_name) = header.thread_info.new_thread_name.clone() {
            self.thread_name = Some(thread_name);
        }
        self.last_thread_ord_id = thread_id;

        // Merge id store
        let mut something_changed = false;
        for (id, (name, r#type)) in header.id_store.tags.iter().enumerate() {
            let id = id as u8;
            if let Some((old_name, old_type)) = self.id_store.get(&id) {
                if old_name.as_ref() != name.deref() || old_type != r#type {
                    something_changed = true;
                    error!("ID store mismatch for thread {:?}#{:?}! ID: {}, Old: {:?}, New: {:?}", self.thread_name, self.thread_id,
                                                id, (old_name, old_type), (name, r#type));
                }
            }
            else {
                something_changed = true;
            }
            self.id_store.insert(id, (Rc::from(name.deref()), *r#type));
        }
        #[cfg(feature="self-tracing")]
        drop(g);

        if something_changed {
            res.push(ThreadParserEvent::EventNamesChanged(self.id_store.clone()));
        }

        #[cfg(feature="self-tracing")]
        let g = sparkles_macro::range_event_start!("Decode raw events");
        let new_events = self.state_machine.decode_many(&events_bytes);
        let new_events_len = new_events.len();
        debug!("Received {new_events_len} events");
        #[cfg(feature="self-tracing")]
        drop(g);

        #[cfg(feature="self-tracing")]
        let g = sparkles_macro::range_event_start!("Parse new events");
        let mut cur_tm = header.start_timestamp;
        let mut first = true;

        let mut parsed_events = Vec::with_capacity(new_events_len / 2);
        let mut unhandled_events = vec![];
        for evt in new_events {
            let mut dif_tm_zero = false;
            if first {
                first = false;
            }
            else {
                let dif_tm = match evt {
                    TracingEvent::Instant(_, dif_tm) => dif_tm,
                    TracingEvent::RangePart(_, dif_tm, _) => dif_tm,
                    TracingEvent::UnnamedRangeEnd(dif_tm, _) => dif_tm,
                    TracingEvent::ForeignRangeEnd(_, dif_tm, _, _) => dif_tm,
                };
                if dif_tm == 0 {
                    dif_tm_zero = true;
                }
                cur_tm += dif_tm;
            }
            if !dif_tm_zero {
                self.zero_diff_cnt = 0;
            }
            else {
                self.zero_diff_cnt += 1;
            }
            if cur_tm > header.end_timestamp {
                warn!("Parsing issue: Timestamp is outside local packet! diff: {}",  cur_tm - header.end_timestamp);
            }

            let Some(tm) = interpolation_points.project_tm(cur_tm) else {
                unhandled_events.push(evt);
                continue;
            };
            let timestamp = tm + self.zero_diff_cnt * 10;
            // Create ParsedEvent
            match evt {
                TracingEvent::Instant(id, _) => {
                    let ev_name = if let Some((ev_name, ev_type)) = self.id_store.get(&id) {
                        if ev_type != &EventType::Instant {
                            error!("Assertion failed: Instant event type is not Instant!");
                        }
                        ev_name.clone()
                    }
                    else {
                        error!("Did not find event name for id: {id}");
                        Rc::from(format!("Unknown Instant {id}"))
                    };
                    let parsed = ParsedEvent::Instant {
                        name_id: id,
                        tm: timestamp
                    };
                    parsed_events.push(parsed);
                }
                TracingEvent::RangePart(id, _, ord_id) => {
                    if let Some((ev_name, ev_type)) = self.id_store.get(&id) {
                        if ev_type == &EventType::Instant {
                            error!("Assertion failed: RangePart event has Instant type!");
                        }
                        else if let EventType::RangeEnd(start_id) = ev_type {
                            if let Some((start_name, start_ev_type)) = self.id_store.get(start_id) {
                                if *start_ev_type != EventType::RangeStart {
                                    error!("Assertion failed: RangePart event has wrong RangeStart type!");
                                }
                                if let Some((ev_id, start_tm)) = self.cur_started_ranges.remove(&ord_id) {
                                    if ev_id != *start_id {
                                        error!("Assertion failed: RangePart event has wrong RangeEnd id!");
                                    }
                                    let parsed = ParsedEvent::Range {
                                        name_id: *start_id,
                                        end_name_id: Some(id),
                                        start: start_tm,
                                        end: timestamp,
                                        start_thread_ord_id: None,
                                    };
                                    parsed_events.push(parsed);
                                }
                                else {
                                    warn!("Did not find start event for RangePart id: {id}");
                                }
                            }
                            else {
                                warn!("Did not find start event for RangePart id: {id}");
                            }
                        }
                        else {
                            // Range start
                            self.cur_started_ranges.insert(ord_id, (id, timestamp));
                        }
                    }
                    else {
                        error!("Did not find event name for id: {id}");
                        let ev_name: Rc<str> = Rc::from(format!("Unknown RangePart {id}"));

                        let parsed = ParsedEvent::Instant {
                            name_id: id,
                            tm: timestamp
                        };
                        parsed_events.push(parsed);
                    };

                }
                TracingEvent::UnnamedRangeEnd(_, ord_id ) => {
                    if let Some(start_info) = self.cur_started_ranges.remove(&ord_id) {
                        if let Some((start_name, ev_type)) = self.id_store.get(&start_info.0) {
                            if *ev_type != EventType::RangeStart {
                                error!("Assertion failed: UnnamedRangeEnd event has non-RangeStart type!");
                            }
                            let parsed = ParsedEvent::Range {
                                name_id: start_info.0,
                                end_name_id: None,
                                start: start_info.1,
                                end: timestamp,
                                start_thread_ord_id: None,
                            };
                            parsed_events.push(parsed);
                        }
                        else {
                            warn!("Did not find start event for UnnamedRangeEnd id: {ord_id}. Skipping...");
                        }
                    }
                    else {
                        warn!("Did not find start event for UnnamedRangeEnd id: {ord_id}. Skipping...");
                    }
                }
                TracingEvent::ForeignRangeEnd(id, _, ord_id, foreign_thread_id) => {
                    self.foreign_range_ends.push(ForeignRangeEnd {
                        event_id: id,
                        timestamp,
                        ord_id,
                        foreign_thread_ord_id: foreign_thread_id,
                    });
                }
            }
        }

        self.stats.new_events(new_events_len, header.start_timestamp, header.end_timestamp);
        self.state_machine.ensure_buf_end();

        if !parsed_events.is_empty() {
            res.push(ThreadParserEvent::NewEvents(parsed_events));
        }

        res
    }

    pub fn thread_info_state(&self) -> ThreadInfoState {
        ThreadInfoState {
            thread_id: self.thread_id,
            thread_name: self.thread_name.clone(),
            thread_ord_id: self.last_thread_ord_id,
        }
    }
}
