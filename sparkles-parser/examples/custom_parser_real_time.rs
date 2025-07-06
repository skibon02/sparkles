use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::packet_decoder::PacketDecoder;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    info!("Waiting for connection to 127.0.0.1:38338...");
    let decoder = PacketDecoder::from_socket("127.0.0.1:38338");
    info!("Connected! Waiting for data...");
    SparklesParser::new().parse_to_end(decoder, |evs, thr, event_names| {
        info!("Got {} events in thread {:?}", evs.len(), thr.thread_name);
        for ev in evs {
            let name = &event_names.get(ev.name_id()).unwrap().0;
            info!("Got event {ev:?}");
        }
    }, |_, _| {}).unwrap();
    info!("Parsing done, client disconnected!");
}
