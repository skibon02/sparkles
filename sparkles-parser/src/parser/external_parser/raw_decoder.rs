use sparkles_core::protocol::packets::ExternalEvents;

pub enum ForeignTracingEvent {
    Instant {
        name_id: u8,
        tm: u64,
    },
    RangePart {
        name_id: u8,
        pairing_id: u8,
        tm: u64,
    },
}

pub fn decode_raw_event(bytes: &[u8], header: &ExternalEvents) -> Option<ForeignTracingEvent> {
    let mut tm_bytes = [0u8; 8];
    tm_bytes[8 - header.bytes_per_timestamp as usize..].copy_from_slice(&bytes[..header.bytes_per_timestamp as usize]);
    let tm = header.start_timestamp + u64::from_be_bytes(tm_bytes);


    let ev_id = bytes[header.bytes_per_timestamp as usize];
    let pairing_id = bytes[header.bytes_per_timestamp as usize + 1];
    if pairing_id == 0 {
        Some(ForeignTracingEvent::Instant{
            name_id: ev_id,
            tm
        })
    }
    else {
        Some(ForeignTracingEvent::RangePart {
            pairing_id,
            name_id: ev_id,
            tm
        })
    }
}