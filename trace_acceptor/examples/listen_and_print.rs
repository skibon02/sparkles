use simple_logger::SimpleLogger;
use trace_acceptor::TraceAcceptor;

fn main() {
    SimpleLogger::new().init().unwrap();
    TraceAcceptor::new().listen();
}