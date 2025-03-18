use log::warn;
use sparkles_core::config::LocalStorageConfig;
use crate::sender::file_sender::FileSenderConfig;
use crate::sender::udp_sender::UdpSenderConfig;

#[derive(Clone, Debug)]
pub struct SparklesConfig {
    /// Capacity of the global storage ring buffer in bytes
    /// 
    /// Default: 50MB
    pub global_capacity: usize,
    
    /// After reaching flush threshold, data will be available for sending (saving to file or sending over UDP)
    /// 
    /// Default: 64KB
    pub flush_threshold: usize,
    
    /// Cleanup threshold for the global storage ring buffer. When the buffer reaches this threshold,
    /// it will start to clean up the oldest events
    ///
    /// Value must be in range [0.0, 1.0]
    /// 
    /// Default: 0.9
    pub cleanup_threshold: f64,
    
    /// Cleanup bottom threshold for the global storage ring buffer. When the buffer reaches this threshold,
    /// it will start to clean up the oldest events
    /// It can only happen if saving data to file or sending over UDP takes too long (rare in practice).
    ///
    /// Value must be in range [0.0, 1.0]
    /// 
    /// Default: 0.7
    pub cleanup_bottom_threshold: f64,
    
    /// Thread-local storage configuration
    pub local_storage_config: LocalStorageConfig,

    pub file_sender_config: Option<FileSenderConfig>,
    pub udp_sender_config: Option<UdpSenderConfig>
}


impl Default for SparklesConfig {
    #[must_use]
    fn default() -> Self {
        Self {
            global_capacity: 50*1024*1024,
            flush_threshold: 64*1024,
            cleanup_threshold: 0.9,
            cleanup_bottom_threshold: 0.7,
            local_storage_config: Default::default(),

            file_sender_config: Some(Default::default()),
            udp_sender_config: None
        }
    }
}

impl SparklesConfig {
    /// Capacity of the global storage ring buffer in bytes
    ///
    /// Default: 50MB
    #[must_use]
    pub fn with_global_capacity(mut self, global_capacity: usize) -> Self {
        self.global_capacity = global_capacity;
        self
    }

    /// After reaching flush threshold, data will be available for sending (saving to file or sending over UDP)
    ///
    /// Default: 64KB
    #[must_use]
    pub fn with_flush_threshold(mut self, flush_threshold: usize) -> Self {
        self.flush_threshold = flush_threshold;
        self
    }

    /// After reaching flush threshold, data will be available to sending (saving to file or sending over UDP)
    /// Value must be in range [0.0, 1.0]
    ///
    /// Default: 0.9
    #[must_use]
    pub fn with_cleanup_threshold(mut self, mut cleanup_threshold: f64) -> Self {
        check_range(&mut cleanup_threshold, "cleanup_threshold");
        self.cleanup_threshold = cleanup_threshold;
        self
    }

    /// Value must be in range [0.0, 1.0]
    ///
    /// Default: 0.7
    #[must_use]
    pub fn with_cleanup_bottom_threshold(mut self, mut cleanup_bottom_threshold: f64) -> Self {
        check_range(&mut cleanup_bottom_threshold, "cleanup_bottom_threshold");
        self.cleanup_bottom_threshold = cleanup_bottom_threshold;
        self
    }

    /// Soft threshold for flushing. Will flush automatically if global buffer is available at the moment.
    ///
    /// Note: This should less than `thread_flush_threshold`
    /// 
    /// Default: 32KB
    #[must_use]
    pub fn with_thread_flush_attempt_threshold(mut self, flush_attempt_threshold: usize) -> Self {
        self.local_storage_config.flush_attempt_threshold = flush_attempt_threshold;
        self
    }

    /// Max capacity of the thread-local storage buffer in bytes. After reaching this threshold,
    /// the buffer will be flushed to the global storage. Thread will be blocked until the flushing operation is finished.
    /// 
    /// Note: This should not be less than thread_flush_threshold. Most of the time you would need to change thread_flush_attempt_threshold.
    ///
    /// Default: 1MB
    #[must_use]
    pub fn with_thread_flush_threshold(mut self, flush_threshold: usize) -> Self {
        self.local_storage_config.flush_threshold = flush_threshold;
        self
    }

    /// Do not save trace data to file
    ///
    /// Default: enabled with Directory(`trace`)
    #[must_use]
    pub fn without_file_sender(mut self) -> Self {
        self.file_sender_config = None;
        self
    }

    /// Save trace data to file
    ///
    /// Default: enabled with Directory(`trace`)
    #[must_use]
    pub fn with_default_file_sender(mut self) -> Self {
        self.file_sender_config = Some(Default::default());
        self
    }

    /// Save trace data to file with configuration
    ///
    /// Default: enabled with Directory(`trace`)
    #[must_use]
    pub fn with_file_sender(mut self, config: FileSenderConfig) -> Self {
        self.file_sender_config = Some(config);
        self
    }

    /// Enable UDP sender with default port.
    /// Default UDP ports: 38338, 38348, 38358
    ///
    /// Default: disabled
    #[must_use]
    pub fn with_default_udp_sender(mut self) -> Self {
        self.udp_sender_config.get_or_insert_default().local_port = None;
        self
    }

    /// Enable UDP sender with custom port.
    /// Do not use it with multicast.
    ///
    /// Default: disabled
    #[must_use]
    pub fn with_udp_sender(mut self, port: u16) -> Self {
        self.udp_sender_config.get_or_insert_default().local_port = Some(port);
        self
    }
    
    
    /// Enable UDP sender with multicast discovery.
    /// Default UDP ports: 38338, 38348, 38358
    /// Do not use it with custom port.
    /// 
    /// Will join multicast on all detected private and loopback interfaces.
    ///
    /// Default: disabled
    #[must_use]
    pub fn with_udp_multicast(mut self) -> Self {
        let config = self.udp_sender_config.get_or_insert_default();
        config.local_port = None;
        config.multicast = true;
        self
    }
}

fn check_range(val: &mut f64, param: &str) {
    if *val > 1.0 {
        warn!("Value {} for SparklesConfig is too big, clamping to 1.0!", param);
        *val = 1.0;
    }

    if *val < 0.0 {
        warn!("Value {} for SparklesConfig is too small, clamping to 0.0!", param);
        *val = 0.0;
    }
}