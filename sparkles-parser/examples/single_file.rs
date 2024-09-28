//! Single file parser
//! 1. Run your application with sparkles. `.sprk` file will be generated.
//! 2. Use this example to parse latest trace file in this folder: `cargo run --release --example single_file filename.sprk`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::env::args;
use log::LevelFilter;
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;


fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    let filename = args().nth(1).unwrap();

    let file = std::fs::File::open(filename).unwrap();
    let mut parser = SparklesParser::default();
    parser.parse_and_save(file).unwrap()
}