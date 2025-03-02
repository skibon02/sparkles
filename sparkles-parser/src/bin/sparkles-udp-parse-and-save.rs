use std::io::Write;
use log::{info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    info!("Waiting for connection to 127.0.0.1:38338...");
    let mut parser = SparklesParser::from_udp_addr("127.0.0.1:38338");
    info!("Connected! Waiting for data...");
    let data = parser.parse_and_convert_to_perfetto().unwrap();
    let mut res_file = std::fs::File::create("trace.perf").unwrap();
    res_file.write_all(&data).unwrap();
    info!("Your `trace.perf` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.");
}