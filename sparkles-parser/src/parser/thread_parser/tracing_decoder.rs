use std::mem;
use ringbuf::consumer::Consumer;
use ringbuf::producer::Producer;
use ringbuf::storage::Heap;
use ringbuf::traits::Observer;
use crate::{EventNameId, PARSER_BUF_SIZE};
use crate::parser::thread_parser::RawTracingEvent;

pub struct StreamFrameDecoder {
    state: ParsingState,
    buf: ringbuf::LocalRb<Heap<u8>>
}

impl Default for StreamFrameDecoder {
    fn default() -> Self {
        Self {
            state: ParsingState::NewFrame,
            buf: ringbuf::LocalRb::new(PARSER_BUF_SIZE)
        }
    }
}

#[derive(Copy,Clone, Default, Debug)]
#[derive(PartialEq)]
pub enum ParsingState {
    #[default]
    NewFrame,
    DifTmLen(EventNameId),

    /// id, dif_tm_len
    DifTm(EventNameId, usize),

    RangeOrdId(Option<EventNameId>, usize, bool),
    RangeTm(Option<EventNameId>, usize, u8, bool),
    ForeignThreadIdLen(Option<EventNameId>, u64, u8),
    ForeignThreadId(Option<EventNameId>, u64, u8, usize)
}

impl StreamFrameDecoder {
    pub fn try_decode_event(&mut self) -> Result<RawTracingEvent, bool> {
        let available_bytes_len = self.buf.occupied_len();

        let (ev, new_state) = match mem::take(&mut self.state) {
            ParsingState::NewFrame if available_bytes_len >= 1 => {
                let ev_id = self.buf.try_pop().unwrap();
                (None, ParsingState::DifTmLen(ev_id))
            }
            ParsingState::DifTmLen(ev) if available_bytes_len >= 1 => {
                let dif_tm_len = self.buf.try_pop().unwrap();

                let is_range_event = dif_tm_len & 0b1000_0000 != 0;
                let is_unnamed_range_end = dif_tm_len & 0b0100_0000 != 0;
                let has_foreign_thread = dif_tm_len & 0b0010_0000 != 0;
                let dif_tm_len = (dif_tm_len & 0b0001_1111) as usize;

                if is_range_event {
                    if is_unnamed_range_end {
                        (None, ParsingState::RangeOrdId(None, dif_tm_len, has_foreign_thread))
                    }
                    else {
                        (None, ParsingState::RangeOrdId(Some(ev), dif_tm_len, has_foreign_thread))
                    }
                }
                else {
                    (None, ParsingState::DifTm(ev, dif_tm_len))
                }
            }
            ParsingState::DifTm(ev, dif_tm_len) if available_bytes_len >= dif_tm_len => {
                let mut buf = [0u8; 8];
                self.buf.pop_slice(&mut buf[..dif_tm_len]);
                let dif_tm = u64::from_le_bytes(buf);
                (Some(RawTracingEvent::Instant(ev, dif_tm)), ParsingState::NewFrame)
            }
            ParsingState::RangeOrdId(ev, dif_tm_len, has_foreign_thread) if available_bytes_len >= 1 => {
                let ord_id = self.buf.try_pop().unwrap();

                (None, ParsingState::RangeTm(ev, dif_tm_len, ord_id, has_foreign_thread))
            }
            ParsingState::RangeTm(ev_id, dif_tm_len, ord_id, has_foreign_thread) if available_bytes_len >= dif_tm_len => {
                let mut buf = [0u8; 8];
                self.buf.pop_slice(&mut buf[..dif_tm_len]);
                let dif_tm = u64::from_le_bytes(buf);

                if has_foreign_thread {
                    (None, ParsingState::ForeignThreadIdLen(ev_id, dif_tm, ord_id))
                } else {
                    let ev = if let Some(id) = ev_id {
                        Some(RawTracingEvent::RangePart(id, dif_tm, ord_id))
                    }
                    else {
                        Some(RawTracingEvent::UnnamedRangeEnd(dif_tm, ord_id))
                    };
                    (ev, ParsingState::NewFrame)
                }
            }
            ParsingState::ForeignThreadIdLen(ev_id, dif_tm, ord_id) if available_bytes_len >= 1 => {
                let foreign_thread_id_bytes_len = self.buf.try_pop().unwrap() as usize;
                (None, ParsingState::ForeignThreadId(ev_id, dif_tm, ord_id, foreign_thread_id_bytes_len))
            }
            ParsingState::ForeignThreadId(ev_id, dif_tm, ord_id, foreign_thread_id_bytes_len) if available_bytes_len >= foreign_thread_id_bytes_len => {
                let mut buf = [0u8; 8];
                self.buf.pop_slice(&mut buf[..foreign_thread_id_bytes_len]);
                let foreign_thread_id = u64::from_le_bytes(buf);

                let ev = Some(RawTracingEvent::ForeignRangeEnd(ev_id, dif_tm, ord_id, foreign_thread_id));
                (ev, ParsingState::NewFrame)
            }
            state => {
                // Not enough bytes
                self.state = state;
                return Err(true)
            }
        };

        self.state = new_state;
        if let Some(ev) = ev {
            Ok(ev)
        }
        else {
            Err(false)
        }
    }

    pub fn decode_many(&mut self, bytes: &[u8]) -> Vec<RawTracingEvent> {
        self.buf.push_slice(bytes);
        let mut events = Vec::new();
        // Try parse as many events as possible
        loop {
            match self.try_decode_event() {
                Ok(ev) => {
                    events.push(ev);
                }
                Err(finish) => {
                    if finish {
                        return events;
                    }
                }
            }
        }
    }

    pub fn ensure_buf_end(&mut self) {
        assert!(self.buf.is_empty());
        assert_eq!(self.state, ParsingState::NewFrame);
    }
}