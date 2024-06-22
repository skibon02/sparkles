use std::cell::RefCell;
use log::info;
use crate::id_mapping::IdStore;
use crate::timestamp::capture_timestamp;

pub struct ThreadLocalStorage {
    buf: Vec<u8>,
    id_store: IdStore
}

impl ThreadLocalStorage {
    pub const fn new()-> Self {
        ThreadLocalStorage {
            buf: Vec::new(),
            id_store: IdStore::new()
        }
    }


    pub fn event(&mut self, hash: u32, string: &str) {
        let (mut dif_pr, now) = capture_timestamp();
        let mut buf = [0; 11];
        let v = self.id_store.insert_and_get_id(hash, string);

        buf[0] = v | 0x80;
        buf[1] = (now >> 8) as u8;
        buf[2] = now as u8;

        let mut ind = 3;
        // While value is 64-16 = 48 bits, we send 7 bits at a time
        while dif_pr > 0 {
            buf[ind] = dif_pr as u8 & 0x7F;

            dif_pr >>= 7;
            ind += 1;
        }

        // Write event packet
        self.buf.extend_from_slice(&buf[..ind])
    }

    pub fn flush(&mut self) -> Box<[u8]> {
        let clone = self.buf.clone().into_boxed_slice();
        self.buf.clear();
        clone
    }
}


pub fn with_thread_local_tracer<F>(f: F)
where F: FnOnce(&mut ThreadLocalStorage) {
    thread_local! {
        static TRACER: RefCell<ThreadLocalStorage> = RefCell::new(ThreadLocalStorage::new());
    }

    TRACER.with_borrow_mut(|tracer| {
        f(tracer)
    });
}