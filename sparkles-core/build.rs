use std::{env, fs};
use std::path::PathBuf;
use cfg_expr::targets::{get_builtin_target_by_triple, Arch, Os};

fn main() -> std::io::Result<()> {
    // 1. Write constants
    let out_dir_path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let out_file_path = out_dir_path.join("consts.rs");

    let proto_version = fs::read_to_string("PROTOCOL_VERSION")?.split(".").map(|s| s.parse::<u8>().unwrap()).collect::<Vec<_>>();
    let proto_version = (proto_version[0], proto_version[1]);

    fs::write(&out_file_path, format!("pub const PROTOCOL_VERSION: (u8, u8) = {proto_version:?};\n"))?;


    // 2. Check target
    let target = env::var("TARGET").unwrap();
    let target = get_builtin_target_by_triple(&target).unwrap();
    
    // Check if force-fallback-impl feature is enabled
    let force_fallback = env::var("CARGO_FEATURE_FORCE_FALLBACK_IMPL").is_ok();
    
    if force_fallback && target.os.is_some() {
        // Priority 1: `force-fallback-impl` feature is enabled and std is available
        println!("cargo:rustc-cfg=use_fallback_timestamp_impl");
    } else if target.os.as_ref().is_some_and(|os| *os == Os::espidf ){
        // Priority 2: ESP-IDF environment
        println!("cargo:rustc-cfg=use_native_timestamp_impl");
    } else if [Arch::x86, Arch::x86_64, Arch::aarch64, Arch::riscv32].contains(&target.arch) {
        // Priority 3: Native architectures (x86, x86_64, aarch64, riscv32)
        println!("cargo:rustc-cfg=use_native_timestamp_impl");
    } else if target.os.is_some() {
        // Priority 4: Other STD environments (e.g., Linux, Windows)
        println!("cargo:rustc-cfg=use_fallback_timestamp_impl");
    }

    println!("cargo::rustc-check-cfg=cfg(use_fallback_timestamp_impl)");
    println!("cargo::rustc-check-cfg=cfg(use_native_timestamp_impl)");

    Ok(())
}