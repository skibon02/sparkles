use core::fmt::Debug;

/// Abstraction for the destination of captured events
///
/// After putting events into the global storage,
/// multiple senders can be used to transfer events to remote client or long-term storage.
trait Sender {
    type Config: Debug;
    fn new(cfg: &Self::Config) -> Self;
    fn send(&self, data: &[u8]);
}