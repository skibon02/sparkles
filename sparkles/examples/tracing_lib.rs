use std::hint::black_box;
use std::thread;
use std::time::Duration;
use simple_logger::SimpleLogger;
use tracing::{Dispatch, dispatcher, instrument};

const N: usize = 1_000_000;




fn calc_sqrt(val: f64) -> f64 {
    val.sqrt()
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
    let _puffin_server = puffin_http::Server::new(&server_addr).unwrap();
    eprintln!("Run this to view profiling data:  puffin_viewer {server_addr}");
    puffin::set_scopes_on(true);

    for _ in 0..100 {
        let mut v = 0.0f64;
        for i in 0..N {
            puffin::profile_scope!("sqrt calc");
            v += calc_sqrt(i as f64 + 234.532);
        }
        puffin::GlobalProfiler::lock().new_frame();
        black_box(v);
    }


}