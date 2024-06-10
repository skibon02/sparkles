use std::sync::Arc;
use simple_logger::SimpleLogger;
use tracer::fifo::AtomicTimestampsRing;
use tracer::granular_buf::GranularBuf;
use tracer::tracing::{IS_FINISHED, Tracer};

pub type LockFreeTracer = Tracer<GranularBuf>;

fn perform_only_tracing(tracer: &LockFreeTracer) {
    for _ in 0..100_000_000 {
        tracer.event(1);
    }
}

fn main() {
    SimpleLogger::new().init().unwrap();
    let tracer = LockFreeTracer::new();

    perform_only_tracing(&tracer);

    IS_FINISHED.store(true, std::sync::atomic::Ordering::Relaxed);
}