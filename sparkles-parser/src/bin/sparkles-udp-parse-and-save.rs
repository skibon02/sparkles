//! Sparkles UDP parser. Connect to your application in real time, capture trace events flow and save result in `trace.perf`
//! 
//! 1. Run your application with sparkles with udp sender enabled.
//! 2. Use this tool to subscribe to live tracing events, and save them to file when your program is finished: `sparkles-udp-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::env;
use std::io::Write;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::packet_decoder::PacketDecoder;
use sparkles_parser::{request_shutdown, SparklesParser};

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    let mut addr = env::args().nth(1).unwrap_or("127.0.0.1:38338".to_string());
    if !addr.contains(':') {
        addr.push_str(":38338");
    }
    
    ctrlc::set_handler(|| {
        request_shutdown();
    }).unwrap();
    
    info!("Waiting for connection to {}...", addr);
    let decoder = PacketDecoder::from_socket(addr);
    info!("Connected! Waiting for data...");
    let data = SparklesParser::new().parse_and_convert_to_perfetto(decoder).unwrap();
    let mut res_file = std::fs::File::create("trace.perf").unwrap();
    res_file.write_all(&data).unwrap();
    info!("Your `trace.perf` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.");
}