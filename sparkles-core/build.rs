use std::{env, fs};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let out_dir_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file_path = out_dir_path.join("consts.rs");

    let proto_version = fs::read_to_string("../PROTOCOL_VERSION")?.split(".").map(|s| s.parse::<u8>().unwrap()).collect::<Vec<_>>();
    let proto_version = (proto_version[0], proto_version[1]);

    fs::write(&out_file_path, format!("pub const PROTOCOL_VERSION: (u8, u8) = {:?};\n", proto_version))?;
    
    Ok(())
}