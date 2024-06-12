use simple_logger::SimpleLogger;
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