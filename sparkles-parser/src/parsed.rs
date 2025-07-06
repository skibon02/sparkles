use crate::TracingEventId;

#[derive(Clone, Debug)]
pub enum ParsedEvent {
    Instant {
        tm: u64,
        name_id: TracingEventId
    },
    Range {
        start: u64,
        end: u64,
        name_id: TracingEventId
    },
    NamedRange {
        name_id: TracingEventId,
        end_name_id: TracingEventId,
        start: u64,
        end: u64,
    }
}

impl ParsedEvent {
    pub fn name_id(&self) -> &TracingEventId {
        match self {
            ParsedEvent::Instant { name_id, .. } => name_id,
            ParsedEvent::Range { name_id, .. } => name_id,
            ParsedEvent::NamedRange { name_id, .. } => name_id,
        }
    }

    pub fn timestamp(&self) -> u64 {
        match self {
            ParsedEvent::Instant { tm, .. } => *tm,
            ParsedEvent::Range { start, .. } => *start,
            ParsedEvent::NamedRange { start, .. } => *start,
        }
    }

}

pub struct ThreadInfoState {
    pub thread_id: Option<u64>,
    pub thread_name: Option<String>,
    pub thread_ord_id: u64
}
