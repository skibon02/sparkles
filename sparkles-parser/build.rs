fn main() -> std::io::Result<()> {
    #[cfg(feature="bin-deps")]
    {
        // 1. Prepare protobuf file for perfetto format
        // https://github.com/google/perfetto/blob/main/protos/perfetto/trace/perfetto_trace.proto
        prost_build::compile_protos(&["perfetto_trace.proto"], &["protos"])?;
    }

    Ok(())
}