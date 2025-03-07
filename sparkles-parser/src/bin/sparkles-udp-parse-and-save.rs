use std::env;
use std::io::Write;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    let mut addr = env::args().nth(1).unwrap_or("127.0.0.1:38338".to_string());
    if !addr.contains(':') {
        addr.push_str(":38338");
    }
    
    info!("Waiting for connection to {}...", addr);
    let mut parser = SparklesParser::from_udp_addr(addr);
    info!("Connected! Waiting for data...");
    let data = parser.parse_and_convert_to_perfetto().unwrap();
    let mut res_file = std::fs::File::create("trace.perf").unwrap();
    res_file.write_all(&data).unwrap();
    info!("Your `trace.perf` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.");
}