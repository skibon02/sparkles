use std::iter::once;
use std::rc::Rc;

#[derive(Clone)]
pub enum ParsedEvent {
    Instant {
        tm: u64,
        name: Rc<str>
    },
    Range {
        start: u64,
        end: u64,
        name: Rc<str>
    },
    NamedRange {
        name: Rc<str>,
        end_name: Rc<str>,
        start: u64,
        end: u64,
    }
}

pub struct ThreadInfoState {
    pub thread_id: u64,
    pub thread_name: String,
}
pub struct ParsedEventGroup {
    ev: ParsedEvent,
    children: Vec<ParsedEventGroup>
}

impl ParsedEventGroup {
    pub(crate) fn iter(&self) -> impl Iterator<Item=ParsedEvent> {
        once(self.ev.clone()).chain(self.children.iter().flat_map(|c| c.iter()))
    }
}
