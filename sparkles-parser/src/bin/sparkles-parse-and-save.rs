//! Interactive file parser
//! 1. Run your application with sparkles with default file sender configuration. `trace` folder will be generated.
//! 2. Use this example to parse latest trace file in this folder: `cargo run --release --example interactive`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::env::args;
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();


    let filename = args().nth(1);
    let found_filename = if let Some(filename) = filename {
        info!("Using path from argument: {}", filename);
        if let Ok(meta) = std::fs::metadata(&filename) {
            if !meta.is_file() {
                error!("Provided path is not a file!");
                return;
            }
        } else {
            error!("Provided path was not found!");
            return;
        }
        filename
    }
    else {
        info!("No argument provided! Using latest trace file from `trace` directory");
        
        // 1. check directory trace
        if let Ok(meta) = std::fs::metadata("trace") {
            if !meta.is_dir() {
                error!("`./trace` is not a directory");
                return;
            }
        } else {
            error!("`trace` directory was not found!");
            return;
        }
        
        // 2. list all files in trace, decode datetime from filename
        let files = std::fs::read_dir("trace").unwrap();
        let mut trace_files = Vec::new();
        for file in files {
            let file = file.unwrap();
            let path = file.path();
            if let Some(filename) = path.file_name() {
                let filename = filename.to_string_lossy();
                if filename.ends_with(".sprk") {
                    let datetime = filename.trim_end_matches(".sprk");
                    let datetime = chrono::NaiveDateTime::parse_from_str(datetime, "%Y-%m-%d_%H-%M-%S");
                    if let Ok(datetime) = datetime {
                        trace_files.push((datetime, path));
                    }
                }
            }
        }

        trace_files.sort_by(|a, b| b.0.cmp(&a.0));
        let Some(latest) = trace_files.first() else {
            error!("No trace files found");
            return;
        };
        
        info!("Found {} trace files: {:?}", trace_files.len(), trace_files);
        info!("Selecting latest file: {:?}", latest.1);

        latest.1.to_string()
    };

    let mut parser = SparklesParser::default();

    // 3. parse the newest file
    let file = std::fs::File::open(found_filename).unwrap();
    parser.convert_file(file).unwrap()
}