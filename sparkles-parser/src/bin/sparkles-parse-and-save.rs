//! Sparkles trace file parser. Take latest file in `trace` folder and prepare `trace.perf` in Perfetto format.
//! 
//! 1. Run your application with sparkles with default file sender configuration. `trace` folder will be generated.
//! 2. Use this tool to parse latest trace file in this folder: `sparkles-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

#[cfg(not(feature="bin-deps"))]
compile_error!("

Sparkles parser binaries should be installed with feature bin-deps:
   cargo install sparkles-parser --features bin-deps

");

#[cfg(feature="bin-deps")]
#[derive(clap::Parser)]
#[command(name = "Sparkles file parser")]
#[command(about = "Parse sparkles trace file and convert it to Perfetto format for viewing in the browser.
Run without arguments to parse the latest trace file in the `trace` folder.")]
struct Cli {
    #[command(flatten)]
    input: InputMode,

    #[arg(short, long, default_value = "trace.perf", help="Output file name")]
    output: String,
    
    #[arg(short, long)]
    version: bool,

    #[arg(short, long)]
    silent: bool,
}


#[cfg(feature="bin-deps")]
#[derive(clap::Args)]
#[group(required = false, multiple = false)]
struct InputMode {
    #[arg(short, long, help="Use this to parse specific file")]
    file: Option<std::path::PathBuf>,
    #[arg(short, long, help="Use this to parse latest file in the provided directory (Default)")]
    dir: Option<std::path::PathBuf>,

}

#[cfg(feature="bin-deps")]
fn main() {
    use simple_logger::SimpleLogger;
    use std::io::Write;
    use std::path::PathBuf;
    use log::{error, info, LevelFilter};
    use sparkles_parser::packet_decoder::PacketDecoder;
    use sparkles_parser::SparklesParser;
    use clap::Parser;


    sparkles_parser::version();
    let cli = Cli::parse();
    if cli.version {
        return;
    }
    if !cli.silent {
        SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
    }
    else {
        SimpleLogger::new().with_level(LevelFilter::Warn).init().unwrap();
    }

    let found_filename = if let Some(filename) = cli.input.file {
        info!("Using file path from argument: {:?}", filename);
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
        let dir = cli.input.dir.unwrap_or("trace".into());
        info!("Searching for the latest trace file in `{:?}` directory", dir);
        
        // 1. check directory trace
        if let Ok(meta) = std::fs::metadata(dir.clone()) {
            if !meta.is_dir() {
                error!("`./{dir:?}` is not a directory");
                return;
            }
        } else {
            error!("`{dir:?}` directory was not found!");
            return;
        }
        
        // 2. list all files in trace, decode datetime from filename
        let files = std::fs::read_dir(dir).unwrap();
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
    let mut res_file = std::fs::File::create(cli.output.clone()).unwrap();
    res_file.write_all(&data).unwrap();
    println!("\nYour `{}` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.", cli.output);
}

#[cfg(not(feature="bin-deps"))]
fn main() {}
