use std::collections::HashMap;
use prost::bytes::BytesMut;
use prost::Message;
use crate::perfetto_format::decl::trace_packet::{Data, OptionalTrustedPacketSequenceId};
use crate::perfetto_format::decl::TracePacket;

mod decl {
    include!(concat!(env!("OUT_DIR"), "/perfetto.protos.rs"));
}

pub struct PerfettoTraceFile {
    trace: decl::Trace,
    proc_descriptor: decl::TrackDescriptor,
    thread_descriptors: HashMap<u64, decl::TrackDescriptor>,

    sequence_id: u32,
    pid: i32,
}

impl PerfettoTraceFile {
    fn new_uuid() -> u64 {
        rand::random()
    }
    fn uuid_for_thread_id(&self, thread_id: u64) -> u64 {
        self.thread_descriptors.get(&(thread_id)).map(|d| d.uuid).unwrap().unwrap()
    }
    pub fn new(proc_name: String, pid: u32) -> Self {
        // emit process descriptor
        let trace = decl::Trace::default();
        let proc_descriptor = decl::TrackDescriptor {
            process: Some(decl::ProcessDescriptor {
                pid: Some(pid as i32),
                process_name: Some(proc_name),
                ..Default::default()
            }),
            uuid: Some(Self::new_uuid()),
            ..Default::default()
        };
        let thread_descriptors = HashMap::new();
        PerfettoTraceFile {
            trace,
            proc_descriptor,
            thread_descriptors,
            sequence_id: Self::new_uuid() as u32,
            pid: pid as i32,
        }
    }

    pub fn add_range_event(&mut self, name: String, thread_id: u64, begin: u64, end: u64) {
        let uuid = self.uuid_for_thread_id(thread_id);

        let mut track_event = decl::TrackEvent::default();
        track_event.name_field = Some(decl::track_event::NameField::Name(name));
        track_event.set_type(decl::track_event::Type::SliceBegin);
        track_event.track_uuid = Some(uuid);

        let mut packet = decl::TracePacket::default();
        packet.timestamp = Some(begin);
        packet.data = Some(Data::TrackEvent(track_event));
        packet.optional_trusted_packet_sequence_id = Some(OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(self.sequence_id));

        self.trace.packet.push(packet);

        let mut track_event = decl::TrackEvent::default();
        track_event.set_type(decl::track_event::Type::SliceEnd);
        track_event.track_uuid = Some(uuid);

        let mut packet = decl::TracePacket::default();
        packet.timestamp = Some(end);
        packet.data = Some(Data::TrackEvent(track_event));
        packet.optional_trusted_packet_sequence_id = Some(OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(self.sequence_id));

        self.trace.packet.push(packet);
    }

    pub fn add_point_event(&mut self, name: String, thread_id: u64, timestamp: u64) {
        let uuid = self.uuid_for_thread_id(thread_id);

        let mut track_event = decl::TrackEvent::default();
        track_event.name_field = Some(decl::track_event::NameField::Name(name));
        track_event.set_type(decl::track_event::Type::Instant);
        track_event.track_uuid = Some(uuid);

        let mut packet = decl::TracePacket::default();
        packet.timestamp = Some(timestamp);
        packet.data = Some(Data::TrackEvent(track_event));
        packet.optional_trusted_packet_sequence_id = Some(OptionalTrustedPacketSequenceId::TrustedPacketSequenceId(self.sequence_id));

        self.trace.packet.push(packet);
    }
    pub fn set_thread_name(&mut self, thread_id: u64, thread_name: String) {
        self.thread_descriptors.entry(thread_id).or_insert_with(|| {
            let proc_uuid = self.proc_descriptor.uuid.unwrap();
            decl::TrackDescriptor {
                thread: Some(decl::ThreadDescriptor {
                    pid: Some(self.pid),
                    tid: Some(thread_id as i32),
                    thread_name: Some(thread_name),
                    ..Default::default()
                }),
                parent_uuid: Some(proc_uuid),
                uuid: Some(Self::new_uuid()),
                ..Default::default()
            }
        });
    }

    pub fn get_bytes(&mut self) -> BytesMut {
        let mut buf = BytesMut::new();
        let proc_packet = TracePacket {
            data: Some(Data::TrackDescriptor(self.proc_descriptor.clone())),
            ..Default::default()
        };
        self.trace.packet.push(proc_packet);

        for (_, thread_descriptor) in self.thread_descriptors.iter() {
            let thread_packet = TracePacket {
                data: Some(Data::TrackDescriptor(thread_descriptor.clone())),
                ..Default::default()
            };
            self.trace.packet.push(thread_packet);
        }
        self.trace.encode(&mut buf).unwrap();
        buf
    }
}