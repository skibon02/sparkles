//! Interactive file parser
//! 1. Run your application with sparkles with default file sender configuration. `trace` folder will be generated.
//! 2. Use this example to parse latest trace file in this folder: `cargo run --release --example interactive`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

use log::{error, info, LevelFilter};
use simple_logger::SimpleLogger;
use sparkles_parser::SparklesParser;

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();

    // 1. check directory trace
    if let Ok(meta) = std::fs::metadata("trace") {
        if !meta.is_dir() {
            error!("trace is not a directory");
            return;
        }
    } else {
        error!("trace directory was not found!");
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

    info!("Found {} trace files: {:?}", trace_files.len(), trace_files);

    let mut parser = SparklesParser::default();
    trace_files.sort_by(|a, b| b.0.cmp(&a.0));

    // 3. parse the newest file
    if let Some((_, path)) = trace_files.first() {
        let file = std::fs::File::open(path).unwrap();
        parser.parse_and_save(file).unwrap()
    }
    else {
        error!("No trace files found");
    }
}