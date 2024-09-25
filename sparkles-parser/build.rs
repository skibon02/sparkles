use std::{env, fs};
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // 1. Prepare consts
    let out_dir_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file_path = out_dir_path.join("consts.rs");

    let encoder_version: u32 = fs::read_to_string("../ENCODER_VERSION")?.parse().unwrap();
    
    fs::write(&out_file_path, format!("pub const ENCODER_VERSION: u32 = {};\n", encoder_version))?;

    // 2. Build protos
    let protoc = prost_build::protoc_from_env();
    if std::process::Command::new(protoc)
        .arg("--version")
        .output()
        .is_err()
    {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("PROTOC", protobuf_src::protoc());
        }
    }
    // https://github.com/google/perfetto/blob/main/protos/perfetto/trace/perfetto_trace.proto
    prost_build::compile_protos(&["perfetto_trace.proto"], &["protos"])?;

    Ok(())
}