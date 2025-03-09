#[cfg(not(feature="bin-deps"))]
compile_error!("

Sparkles parser binaries should be installed with feature bin-deps:
   cargo install sparkles-parser --features bin-deps

");

fn main() -> std::io::Result<()> {
    // 1. Prepare protobuf file for perfetto format
    // https://github.com/google/perfetto/blob/main/protos/perfetto/trace/perfetto_trace.proto
    prost_build::compile_protos(&["perfetto_trace.proto"], &["protos"])?;

    Ok(())
}