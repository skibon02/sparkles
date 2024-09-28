use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;

/// Abstraction for the destination of captured events
///
/// After putting events into the global storage,
/// multiple senders can be used to transfer events to remote client or long-term storage.
pub trait Sender {
    fn send(&mut self, data: &[u8]);
}

pub trait ConfiguredSender: Sender + Sized {
    type Config: Debug;
    fn new(cfg: &Self::Config) -> Option<Self>;
    fn new_default() -> Option<Self>
    where
        Self::Config: Default
    {
        Self::new(&Self::Config::default())
    }
}

/// Storage for multiple senders, which is also a sender.
#[derive(Default)]
pub struct SenderChain {
    senders: Vec<Box<dyn Sender>>,
}

impl SenderChain {
    pub fn with_sender<T: Sender + 'static>(&mut self, sender: T) {
        self.senders.push(Box::new(sender))
    }
}

impl Sender for SenderChain {
    fn send(&mut self, data: &[u8]) {
        for sender in self.senders.iter_mut() {
            sender.send(data);
        }
    }
}