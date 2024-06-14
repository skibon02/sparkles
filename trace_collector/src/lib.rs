use std::net::UdpSocket;

pub mod thread_local_storage;
mod timestamp;

pub fn event(v: u8) {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        tracer.event(v);
    });
}

pub fn flush() {
    thread_local_storage::with_thread_local_tracer(|tracer| {
        let udp_socket = UdpSocket::bind("127.0.0.1:4303").unwrap();
        udp_socket.connect("127.0.0.1:4302").unwrap();

        let bytes = tracer.flush();
        //split bytes into chunks of 1024 bytes
        let mut chunks = bytes.chunks(5000);
        for chunk in chunks {
            udp_socket.send(chunk).unwrap();
        }
    });
}