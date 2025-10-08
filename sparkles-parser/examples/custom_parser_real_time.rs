use indexmap::IndexMap;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::packet_decoder::PacketDecoder;
use sparkles_parser::{SparklesParser, SparklesParserEvent};
use sparkles_parser::parser::thread_parser::{EventNamesStore, ThreadParserEvent};

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    info!("Waiting for connection to 127.0.0.1:38338...");
    let decoder = PacketDecoder::from_socket("127.0.0.1:38338");
    info!("Connected! Waiting for data...");

    let mut per_thread_event_names: IndexMap<u64, EventNamesStore> = IndexMap::new();
    SparklesParser::new().parse_to_end(decoder, |event| {
        if let SparklesParserEvent::ThreadParserEvent(evt, info) = event {
            match evt {
                ThreadParserEvent::NewThreadName(name) => {
                    info!("Thread {:?} got name: {name}", info.thread_ord_id);
                }
                ThreadParserEvent::NewEvents(evs) => {
                    info!("Got {} events in thread {:?}", evs.len(), info.thread_name);
                    let event_names = per_thread_event_names.entry(info.thread_ord_id).or_default();
                    for ev in evs {
                        let name = &event_names.get(ev.name_id()).unwrap().0;
                        info!("Got event {ev:?}");
                    }
                }
                ThreadParserEvent::EventNamesChanged(event_names) => {
                    per_thread_event_names.entry(info.thread_ord_id).or_default().extend(event_names.into_iter());
                }
            }
        }
    }).unwrap();
    info!("Parsing done, client disconnected!");
}
