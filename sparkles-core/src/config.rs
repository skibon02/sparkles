#[derive(Copy, Clone, Debug)]
pub struct LocalStorageConfig {
    /// Soft threshold for flushing. Will flush automatically only if global buffer is available at the moment.
    /// 
    /// Default: 32KB
    pub flush_attempt_threshold: usize,
    /// Max capacity of the thread-local storage buffer in bytes. After reaching this threshold,
    /// the buffer will be flushed to the global storage. Thread will be blocked until the flushing operation is finished.
    /// 
    /// Default: 1MB
    pub flush_threshold: usize,
}

impl LocalStorageConfig {
    #[must_use]
    pub const fn default() -> Self {
        Self {
            flush_attempt_threshold: 32*1024,
            flush_threshold: 1024*1024,
        }
    }
}

impl Default for LocalStorageConfig {
    #[must_use]
    fn default() -> Self {
        Self::default()
    }
}