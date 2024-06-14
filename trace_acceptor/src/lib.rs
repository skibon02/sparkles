use std::net::UdpSocket;
use log::info;

pub struct TraceAcceptor {
    stream_parser: ParsingStateMachine,
}

impl TraceAcceptor {
    pub fn new() -> Self {
        Self {
            stream_parser: ParsingStateMachine::EventId
        }
    }

    pub fn listen(&mut self)  {
        let udp_socket = UdpSocket::bind("127.0.0.1:4302").unwrap();

        let mut buf = [0; 10_000];

        let mut total_pr = 0;
        let mut first = true;
        info!("Listening for incoming packets...");
        loop {
            let c = udp_socket.recv(&mut buf).unwrap();
            if c == 0 {
                break;
            }
            let new_events = self.stream_parser.parse_many(&buf[..c]);
            info!("Parsed {} events", new_events.len());
            for event in new_events {
                if first {
                    first = false;
                }
                else {
                    total_pr += event.2;
                }
            }
            info!("Total pr: {}", total_pr);
        }
        info!("Disconnected!");

    }
}

/// event, timestamp end (cpu cycles), dif_pr (48 bits)
#[derive(Debug)]
pub struct TracingEvent(u8, u16, u64);

#[derive(Copy,Clone, Default)]
pub enum ParsingStateMachine {
    #[default]
    EventId,
    TimestampLow(u8),
    TimestampHigh(u8, u8),
    TimestampPrOrEventId(u8, u16, u64, u8),
}

impl ParsingStateMachine {
    pub fn next_byte(&mut self, b: u8) -> Option<TracingEvent> {
        match *self {
            ParsingStateMachine::EventId => {
                *self = ParsingStateMachine::TimestampLow(b & 0x7F);
                None
            }
            ParsingStateMachine::TimestampLow(event_id) => {
                *self = ParsingStateMachine::TimestampHigh(event_id, b);
                None
            }
            ParsingStateMachine::TimestampHigh(event_id, low) => {
                *self = ParsingStateMachine::TimestampPrOrEventId(event_id, low as u16 | (b as u16) << 8, 0, 0);
                None
            }
            ParsingStateMachine::TimestampPrOrEventId(event_id, now, pr, pr_shift) => {
                if b & 0x80 != 0 {
                    // new event start, finalize current event
                    *self = ParsingStateMachine::TimestampLow(b & 0x7F);
                    Some(TracingEvent(event_id, now, pr))
                }
                else {
                    *self = ParsingStateMachine::TimestampPrOrEventId(event_id, now, (pr << 6) | (b as u64 & 0x3F), pr_shift + 6);
                    None
                }
            }
        }
    }

    pub fn parse_many(&mut self, bytes: &[u8]) -> Vec<TracingEvent> {
        bytes.iter().flat_map(|b| self.next_byte(*b)).collect()
    }
}