use std::hint::black_box;
use std::thread;
use std::time::{Duration, Instant};
use log::info;
use simple_logger::SimpleLogger;

const N: usize = 1_000;

fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
    let _puffin_server = puffin_http::Server::new(&server_addr).unwrap();
    eprintln!("Run this to view profiling data:  puffin_viewer --url {server_addr}");
    puffin::set_scopes_on(true);

    //wait for client connection
    thread::sleep(Duration::from_secs(3));

    info!("Starting workload...");
    let start = Instant::now();
    for _ in 0..100 {
        let mut v = 0.0f64;
        for i in 0..N {
            puffin::profile_scope!("sqrt calc");
            v += calc_sqrt(i as f64 + 234.532);
        }
        puffin::GlobalProfiler::lock().new_frame();
        black_box(v);
    }
    let dur = start.elapsed().as_nanos() as f64 / 2.0 / (100 * N) as f64;
    info!("Finished! waiting for tracer send...");
    info!("Each event took {:?} ns", dur);

}