//! Sparkles trace file parser. Take latest file in `trace` folder and prepare `trace.perf` in Perfetto format.
//! 
//! 1. Run your application with sparkles with default file sender configuration. `trace` folder will be generated.
//! 2. Use this tool to parse latest trace file in this folder: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use std::env::args;
use std::io::Write;
use std::path::PathBuf;
use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::packet_decoder::PacketDecoder;
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
        PathBuf::from(filename)
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

        latest.1.clone()
    };

    let file = std::fs::File::open(found_filename).unwrap();
    let decoder = PacketDecoder::from_stream(file);

    // 3. parse the newest file
    info!("Begin parsing...");
    let data = SparklesParser::new().parse_and_convert_to_perfetto(decoder).unwrap();
    let mut res_file = std::fs::File::create("trace.perf").unwrap();
    res_file.write_all(&data).unwrap();
    info!("Your `trace.perf` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.");
}