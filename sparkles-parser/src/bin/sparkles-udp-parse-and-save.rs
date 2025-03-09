//! Sparkles UDP parser. Connect to your application in real time, capture trace events flow and save result in `trace.perf`
//! 
//! 1. Run your application with sparkles with udp sender enabled.
//! 2. Use this tool to subscribe to live tracing events, and save them to file when your program is finished: `sparkles-udp-parse-and-save`
//! 3. Go to https://ui.perfetto.dev/ and drag'n'drop generated `trace.perf` file

#[cfg(not(feature="bin-deps"))]
compile_error!("

Sparkles parser binaries should be installed with feature bin-deps:
   cargo install sparkles-parser --features bin-deps

");

#[cfg(feature="bin-deps")]
#[derive(clap::Parser)]
#[command(name = "Sparkles UDP parser")]
#[command(about = "Collect sparkles trace data from UDP socket and save it in Perfetto format for viewing in the browser.")]
struct Cli {
    #[arg(short, long, default_value = "trace.perf", help = "Output file name")]
    output: String,

    #[arg(default_value="127.0.0.1:38338", help = "Remote UDP address and port. Default port is 38338 if not specified")]
    addr: String,

    #[arg(short, long)]
    version: bool,

    #[arg(short, long)]
    silent: bool,
}

#[cfg(feature="bin-deps")]
fn main() {
    use std::io::Write;
    use clap::Parser;
    use log::{info, LevelFilter};
    use simple_logger::SimpleLogger;
    use sparkles_parser::packet_decoder::PacketDecoder;
    use sparkles_parser::{is_shutting_down, request_shutdown, SparklesParser};


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

    let mut addr = cli.addr;
    if !addr.contains(':') {
        addr.push_str(":38338");
    }
    
    ctrlc::set_handler(|| {
        request_shutdown();
    }).unwrap();
    
    info!("Waiting for connection to {}...", addr);
    let decoder = PacketDecoder::from_socket(addr);
    if is_shutting_down() {
        return;
    }
    info!("Connected! Waiting for data...");
    let data = SparklesParser::new().parse_and_convert_to_perfetto(decoder).unwrap();
    let mut res_file = std::fs::File::create(cli.output.clone()).unwrap();
    res_file.write_all(&data).unwrap();
    println!("\nYour `{}` is ready! Now, navigate to https://ui.perfetto.dev/ and drag'n'drop the file onto the page.", cli.output);
}

#[cfg(not(feature="bin-deps"))]
fn main() {}
