use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use log::debug;
use sparkles_core::protocol::packets::PacketType;
use sparkles_core::protocol::sender::{ConfiguredSender, Sender};

pub(crate) struct FileSender {
    file: File
}

#[derive(Debug, Clone)]
pub enum FileSenderConfig {
    /// Filename will be generated automatically in the format of `{dir_name}/%Y-%m-%d_%H-%M-%S.sprk`
    Directory(PathBuf),
    /// 0: Output file name. Will overwrite file if already exists.
    SingleFile(PathBuf),
}

impl Default for FileSenderConfig {
    fn default() -> Self {
        Self::Directory(PathBuf::from("trace"))
    }
}
impl Sender for FileSender {
    fn send_packet(&mut self, packet_type: PacketType, data: &[&[u8]]) {
        self.file.write_all(&packet_type.header()).unwrap();
        let full_size = data.iter().fold(0, |prev, sub| prev + sub.len()) as u32;
        let size_bytes = full_size.to_be_bytes();
        self.file.write_all(&size_bytes).unwrap();
        for d in data {
            self.file.write_all(d).unwrap();
        }
    }
}

impl ConfiguredSender for FileSender {
    type Config = FileSenderConfig;
    fn new(cfg: &Self::Config) -> Option<Self> {
        // Create log file
        match cfg {
            FileSenderConfig::Directory(dir) => {
                if fs::metadata(dir).is_err() {
                    debug!("[sparkles] Creating output directory...");
                    fs::create_dir(dir).ok()?;
                }

                let now = chrono::Local::now();
                let filename = format!("{}/{}.sprk", dir.to_str().unwrap(), now.format("%Y-%m-%d_%H-%M-%S"));
                debug!("[sparkles] Creating output file: {}", filename);
                let file = File::create(filename).ok()?;

                Some(Self {
                    file
                })
            }
            FileSenderConfig::SingleFile(filename) => {
                let file = File::create(filename).ok()?;

                Some(Self {
                    file
                })
            }
        }
    }
}