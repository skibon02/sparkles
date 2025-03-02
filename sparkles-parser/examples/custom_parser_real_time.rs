use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Debug).init().unwrap();

    info!("Waiting for connection to 127.0.0.1:38338...");
    let mut parser = SparklesParser::from_udp_addr("127.0.0.1:38338");
    info!("Connected! Waiting for data...");
    parser.parse_to_end(|ev, thr| {
        
    }).unwrap();
    info!("Parsing done, client disconnected!");
}
