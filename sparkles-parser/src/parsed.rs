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
        name_id: TracingEventId,
        end_name_id: Option<TracingEventId>,
        start_thread_ord_id: Option<u64>,
    },
}

#[derive(Clone, Debug)]
pub enum ParsedExternalEvent {
    Instant {
        tm: u64,
        name_id: TracingEventId,
    },
    Range {
        start: u64,
        end: u64,
        name_id: TracingEventId,
        end_name_id: Option<TracingEventId>,
    },
}

impl ParsedExternalEvent {
    pub fn name_id(&self) -> &TracingEventId {
        match self {
            ParsedExternalEvent::Instant { name_id, .. } => name_id,
            ParsedExternalEvent::Range { name_id, .. } => name_id,
        }
    }

    pub fn timestamp(&self) -> u64 {
        match self {
            ParsedExternalEvent::Instant { tm, .. } => *tm,
            ParsedExternalEvent::Range { start, .. } => *start,
        }
    }
}

impl ParsedEvent {
    pub fn name_id(&self) -> &TracingEventId {
        match self {
            ParsedEvent::Instant { name_id, .. } => name_id,
            ParsedEvent::Range { name_id, .. } => name_id,
        }
    }

    pub fn timestamp(&self) -> u64 {
        match self {
            ParsedEvent::Instant { tm, .. } => *tm,
            ParsedEvent::Range { start, .. } => *start,
        }
    }
}

pub struct ThreadInfoState {
    pub thread_id: Option<u64>,
    pub thread_name: Option<String>,
    pub thread_ord_id: u64
}
