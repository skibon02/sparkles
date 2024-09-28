use std::fs;
use std::fs::File;
use std::io::Write;
use log::debug;
use sparkles_core::sender::{ConfiguredSender, Sender};

pub(crate) struct FileSender {
    file: File
}

#[derive(Debug, Default, Clone)]
pub struct FileSenderConfig {
    /// Output file name. Will overwrite file if already exists.
    /// If not set, filename will be generated automatically in the format of `trace/%Y-%m-%d_%H-%M-%S.sprk`
    pub output_filename: Option<String>
}
impl Sender for FileSender {
    fn send(&mut self, data: &[u8]) {
        self.file.write_all(data).unwrap();
    }
}

impl ConfiguredSender for FileSender {
    type Config = FileSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        // Create log file
        let res = if let Some(filename) = cfg.output_filename.clone() {
            let file = File::create(filename).ok()?;

            Self {
                file
            }
        }
        else {
            let dir = "trace";
            if fs::metadata(dir).is_err() {
                debug!("[sparkles] Creating output directory...");
                fs::create_dir(dir).ok()?;
            }

            let now = chrono::Local::now();
            let filename = format!("{}/{}.sprk", dir, now.format("%Y-%m-%d_%H-%M-%S"));
            debug!("[sparkles] Creating output file: {}", filename);
            let file = File::create(filename).ok()?;

            Self {
                file
            }
        };

        Some(res)
    }
}