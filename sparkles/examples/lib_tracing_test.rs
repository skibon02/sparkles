use tracing::info;
use tracing_perfetto::PerfettoLayer;
use tracing_subscriber::{fmt, Registry};
use tracing_subscriber::fmt::format::Format;
use tracing_subscriber::layer::SubscriberExt;

fn main() -> anyhow::Result<()> {
    let file = std::env::temp_dir().join("test.pftrace");
    let perfetto_layer = PerfettoLayer::new(std::sync::Mutex::new(std::fs::File::create(&file)?))
        .with_debug_annotations(true);

    let fmt_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .event_format(Format::default().with_thread_ids(true))
        .with_span_events(fmt::format::FmtSpan::FULL);

    let subscriber = Registry::default().with(perfetto_layer).with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    info!("start");

    test();

    info!("end");

    Ok(())
}

#[tracing::instrument]
fn test() {
    info!("Inside function");
}