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
    pub thread_id: Option<u64>,
    pub thread_name: Option<String>,
    pub thread_ord_id: u64
}
