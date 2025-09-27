use std::rc::Rc;
use crate::{EventNameId, ExternalEventNameId};

#[derive(Clone, Debug)]
pub enum ParsedEvent {
    Instant {
        tm: u64,
        name_id: EventNameId
    },
    Range {
        start: u64,
        end: u64,
        name_id: EventNameId,
        end_name_id: Option<EventNameId>,
        start_thread_ord_id: Option<u64>,
    },
}

impl ParsedEvent {
    pub fn name_id(&self) -> &EventNameId {
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

#[derive(Clone, Debug)]
pub enum ParsedExternalEvent {
    Instant {
        tm: u64,
        name_id: ExternalEventNameId,
    },
    Range {
        start: u64,
        end: u64,
        name_id: ExternalEventNameId,
        end_name_id: Option<ExternalEventNameId>,
    },
}

impl ParsedExternalEvent {
    pub fn name_id(&self) -> &ExternalEventNameId {
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


pub struct ThreadInfo {
    pub thread_id: Option<u64>,
    pub thread_name: Option<String>,
    pub thread_ord_id: u64
}

pub struct ExternalChannelInfo {
    pub ext_ord_id: u32,
    pub channel_name: Option<Rc<str>>,
}