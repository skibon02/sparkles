#[derive(Copy, Clone, Debug)]
pub struct LocalStorageConfig {
    /// Max capacity of the thread-local storage buffer in bytes. After reaching this threshold,
    /// the buffer will be flushed to the global storage
    pub flush_threshold: usize,
}

impl LocalStorageConfig {
    #[must_use]
    pub const fn default() -> Self {
        Self {
            flush_threshold: 10*1024,
        }
    }
}

impl Default for LocalStorageConfig {
    #[must_use]
    fn default() -> Self {
        Self::default()
    }
}