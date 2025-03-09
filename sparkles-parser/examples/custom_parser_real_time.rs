use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::packet_decoder::PacketDecoder;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    info!("Waiting for connection to 127.0.0.1:38338...");
    let decoder = PacketDecoder::from_socket("127.0.0.1:38338");
    info!("Connected! Waiting for data...");
    SparklesParser::new().parse_to_end(decoder, |ev, thr| {
        info!("Got event {:?}", ev);
    }).unwrap();
    info!("Parsing done, client disconnected!");
}
