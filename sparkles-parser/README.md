# Sparkles parser

Library for parsing sparkles byte stream.

After trace information is recorded to `*.sprk` file, you need to convert it to the perfetto format.

## ✧ Installation
```bash
cargo install sparkles-parser
```

Two binaries are included
## ✧ interactive
Used for default sparkles configuration. Opens director `trace` and begin parsing the latest saved file.

The result is saved with filename `trace.perf`

## ✧ single-file
A single command line argument is required: sparkles trace filename.

The result is saved with filename `trace.perf`

## ✧ How to use
Run one of described binaries to convert `*.sprk` sparkles event stream file to the Perfetto protobuf format.
Navigate to https://ui.perfetto.dev/ to open the generated file.
