use std::fmt::Display;
use std::ops::Deref;
use std::rc::Rc;
use crate::TracingEventId;

#[derive(Clone, Debug)]
pub enum ParsedEvent {
    Instant {
        tm: u64,
        name: EventName
    },
    Range {
        start: u64,
        end: u64,
        name: EventName
    },
    NamedRange {
        name: EventName,
        end_name: EventName,
        start: u64,
        end: u64,
    }
}

#[derive(Clone, Debug)]
pub struct EventName {
    id: TracingEventId,
    name: Rc<str>,
}

impl EventName {
    pub fn new(id: TracingEventId, name: &Rc<str>) -> Self {
        Self {
            id,
            name: name.clone()
        }
    }
}

impl Deref for EventName {
    type Target = Rc<str>;
    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

impl Display for EventName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub struct ThreadInfoState {
    pub thread_id: Option<u64>,
    pub thread_name: Option<String>,
    pub thread_ord_id: u64
}
