# Sparkles parser

Library for parsing sparkles byte stream.

Two binaries are included
## ✧ interactive
Used for default sparkles configuration. Opens director `trace` and begin parsing the latest saved file.

The result is saved with filename `trace.perf`

## ✧ single_file
A single command line argument is required: sparkles trace filename.

The result is saved with filename `trace.perf`

## ✧ How to use
Run one of described binaries to convert `*.sprk` sparkles event stream file to the Perfetto protobuf format.
Navigate to https://ui.perfetto.dev/ to open the generated file.
