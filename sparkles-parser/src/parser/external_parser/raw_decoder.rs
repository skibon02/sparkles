use sparkles_core::protocol::packets::ExternalEvents;

pub enum RawExternalTracingEvent {
    Instant {
        name_id: u16,
        raw_tm: u64,
    },
    RangePart {
        name_id: u16,
        pairing_id: u8,
        is_end: bool,
        raw_tm: u64,
    },
}

impl RawExternalTracingEvent {
    pub fn raw_timestamp(&self) -> u64 {
        match self {
            RawExternalTracingEvent::Instant { raw_tm, .. } => *raw_tm,
            RawExternalTracingEvent::RangePart { raw_tm, .. } => *raw_tm,
        }
    }

    pub fn name_id(&self) -> u16 {
        match self {
            RawExternalTracingEvent::Instant { name_id, .. } => *name_id,
            RawExternalTracingEvent::RangePart { name_id, .. } => *name_id,
        }
    }
}

pub fn decode_raw_event(bytes: &[u8], header: &ExternalEvents) -> Option<RawExternalTracingEvent> {
    let mut tm_bytes = [0u8; 8];
    let bytes_per_timestamp = header.bytes_per_timestamp as usize;
    tm_bytes[..bytes_per_timestamp].copy_from_slice(&bytes[..bytes_per_timestamp]);
    let tm = header.start_timestamp + u64::from_le_bytes(tm_bytes);


    let ev_id = u16::from_be_bytes([bytes[bytes_per_timestamp], bytes[bytes_per_timestamp + 1]]);
    let pairing_id = bytes[bytes_per_timestamp + 2];
    if pairing_id == 0 {
        Some(RawExternalTracingEvent::Instant{
            name_id: ev_id,
            raw_tm: tm
        })
    }
    else {
        let is_end = pairing_id & 0x80 != 0;
        let pairing_id = pairing_id & 0x7F;
        Some(RawExternalTracingEvent::RangePart {
            name_id: ev_id,
            raw_tm: tm,
            pairing_id,
            is_end,
        })
    }
}