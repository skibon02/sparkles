# Sparkles parser

Library for parsing [sparkles](https://crates.io/crates/sparkles) trace data.

Tracing information can be either saved to file or transmitted over UDP socket.
These two flavors of sparkles tracing data protocol can be parsed and converted to Perfetto format file using this library.

Files compatible with Perfetto format can be easily viewed in browser on the [Perfetto](https://ui.perfetto.dev) website.

## ✧ Installation
```bash
cargo install sparkles-parser --features bin-deps
```

Two binaries are included
## ✧ sparkles-parse-and-save
Used for default sparkles configuration. 
It will open directory `trace` and parse the latest saved file, or parse specific file provided as an argument.

The result is saved with filename `trace.perf`

## ✧ sparkles-udp-parse-and-save
Application need to enable udp sender in config first.
Connect to running application and collect tracing data over UDP until it is finished.
If you want to interrupt tracing data collection early, just press Ctrl-C.

The result is saved with filename `trace.perf`

## ✧ How to use
Run one of described binaries to generate Perfetto protobuf format file.
Navigate to https://ui.perfetto.dev/ and drag'n'drop generated file.

## Crate features
✧ **local-packet-bounds** - Add received packet bounds to the result  
✧ **self-tracing** - Add event receive and parse events to the result  
✧ **perfetto** - Add saving in perfetto format method to the library
✧ **bin-deps** - Should not be used if you are using library

｡ﾟﾟ･｡･ﾟﾟ｡  
ﾟ。SkyGrel19 ✨  
　ﾟ･｡･
