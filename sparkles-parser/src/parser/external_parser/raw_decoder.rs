use sparkles_core::protocol::packets::ExternalEvents;

pub enum RawForeignTracingEvent {
    Instant {
        name_id: u8,
        raw_tm: u64,
    },
    RangePart {
        name_id: u8,
        pairing_id: u8,
        is_end: bool,
        raw_tm: u64,
    },
}

pub fn decode_raw_event(bytes: &[u8], header: &ExternalEvents) -> Option<RawForeignTracingEvent> {
    let mut tm_bytes = [0u8; 8];
    tm_bytes[8 - header.bytes_per_timestamp as usize..].copy_from_slice(&bytes[..header.bytes_per_timestamp as usize]);
    let tm = header.start_timestamp + u64::from_be_bytes(tm_bytes);


    let ev_id = bytes[header.bytes_per_timestamp as usize];
    let pairing_id = bytes[header.bytes_per_timestamp as usize + 1];
    if pairing_id == 0 {
        Some(RawForeignTracingEvent::Instant{
            name_id: ev_id,
            raw_tm: tm
        })
    }
    else {
        let is_end = pairing_id & 0x80 != 0;
        let pairing_id = pairing_id & 0x7F;
        Some(RawForeignTracingEvent::RangePart {
            name_id: ev_id,
            raw_tm: tm,
            pairing_id,
            is_end,
        })
    }
}