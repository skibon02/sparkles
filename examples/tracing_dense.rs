use std::sync::Arc;
use simple_logger::SimpleLogger;
use tracer::fifo::AtomicTimestampsRing;
use tracer::granular_buf::GranularBuf;
use tracer::tracing::{Tracer};
use tracer::r#impl::std_impl::LockFreeTracer;

fn perform_only_tracing(tracer: &LockFreeTracer) {
    for _ in 0..100_000_000 {
        tracer.event(1);
    }
}

fn main() {
    SimpleLogger::new().init().unwrap();
    let tracer = LockFreeTracer::new();

    perform_only_tracing(&tracer);
}