use sparkles_core::headers::{LocalPacketHeader, SparklesEncoderInfo};
use sparkles_core::sender::Sender;

pub fn send_encoder_info_packet(sender: &mut impl Sender, sparkles_encoder_info: SparklesEncoderInfo) {
    let encoded_info = bincode::serialize(&sparkles_encoder_info).unwrap();
    sender.send(&[0x00]);
    sender.send(&(encoded_info.len() as u64).to_le_bytes());
    sender.send(&encoded_info);
}

pub fn send_data_bytes(sender: &mut impl Sender, slice1: &[u8], slice2: &[u8]) {
    sender.send(&[0x01]);
    let total_len = (slice1.len() + slice2.len()) as u64;
    let total_len_bytes = total_len.to_le_bytes();
    sender.send(&total_len_bytes);
    sender.send(slice1);
    sender.send(slice2);
}

pub fn send_failed_page_headers(sender: &mut impl Sender, failed_pages: &[LocalPacketHeader]) {
    for failed_msr_page in failed_pages {
        let header = bincode::serialize(&failed_msr_page).unwrap();
        let header_len = (header.len() as u64).to_le_bytes();
        sender.send(&[0x02]);
        sender.send(&header_len);
        sender.send(&header);
    }
}

pub fn send_timestamp_freq(sender: &mut impl Sender, ticks_per_sec: u64) {
    sender.send(&[0x03]);
    let bytes = ticks_per_sec.to_le_bytes();
    sender.send(&bytes);
}