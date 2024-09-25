use std::{env, fs};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {

    let out_dir_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file_path = out_dir_path.join("consts.rs");

    let encoder_version: u32 = fs::read_to_string("../ENCODER_VERSION")?.parse().unwrap();
    
    fs::write(&out_file_path, format!("pub const ENCODER_VERSION: u32 = {};\n", encoder_version))?;
    
    Ok(())
}