#[derive(Copy, Clone, Debug)]
pub struct SparklesConfig {
    /// Capacity of the global storage ring buffer in bytes
    pub global_capacity: usize,
    /// Value should be in range [0.0, 1.0]
    pub flush_threshold: f64,
    /// Cleanup threshold for the global storage ring buffer. When the buffer reaches this threshold,
    /// it will start to clean up the oldest events
    ///
    /// Value should be in range [0.0, 1.0]
    pub cleanup_threshold: f64,
    /// Cleanup bottom threshold for the global storage ring buffer. When the buffer reaches this threshold,
    /// it will start to clean up the oldest events
    ///
    /// Value should be in range [0.0, 1.0]
    pub cleanup_bottom_threshold: f64,
    /// Thread-local storage configuration
    pub local_storage_config: LocalStorageConfig,
}

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
impl Default for SparklesConfig {
    #[must_use]
    fn default() -> Self {
        Self {
            global_capacity: 50*1024*1024,
            flush_threshold: 0.1,
            cleanup_threshold: 0.9,
            cleanup_bottom_threshold: 0.7,
            local_storage_config: Default::default()
        }
    }
}

impl SparklesConfig {
    #[must_use]
    pub fn with_global_capacity(mut self, global_capacity: usize) -> Self {
        self.global_capacity = global_capacity;
        self
    }

    #[must_use]
    pub fn with_flush_threshold(mut self, flush_threshold: f64) -> Self {
        self.flush_threshold = flush_threshold;
        self
    }

    #[must_use]
    pub fn with_cleanup_threshold(mut self, cleanup_threshold: f64) -> Self {
        self.cleanup_threshold = cleanup_threshold;
        self
    }

    #[must_use]
    pub fn with_cleanup_bottom_threshold(mut self, cleanup_bottom_threshold: f64) -> Self {
        self.cleanup_bottom_threshold = cleanup_bottom_threshold;
        self
    }

    #[must_use]
    pub fn with_thread_flush_threshold(mut self, flush_threshold: usize) -> Self {
        self.local_storage_config.flush_threshold = flush_threshold;
        self
    }
}