# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [0.1.4] - 2024-09-28
- [sparkles] Added file saving support
- [sparkles] **[WIP]** Added UDP sender support with configuration 
- [sparkles] Add sender config to SparklesConfig
- [sparkles] Removed TCP support
- [sparkles] Add `self-tracing` feature
- [sparkles] Improve flushing performance: Largely reduce TLS operations blocking during global storage flushing.
- [sparkles] Fix potential `ticks_per_sec` overflow
- [sparkles-core] aarch64 timestamps support
- [sparkles-core] Add `try_flush` and `is_buffer_available` to the global storage ref.
- [sparkles-core] Add soft flushing threshold for thread local storage.
- [encoder format] Add ticks_per_sec packet type
- [sparkles-parser] Add small offset when several events recorded with the same timestamp


## [0.1.3] - 2024-09-23
- [sparkles-core] New: Add configuration structures: `SparklesConfig` and `LocalStorageConfig`
- [sparkles-core] Fix: Add distinction between events with the same name but different categories
- [sparkles-core] `ThreadInfo` is now a part of `LocalPacketHeader`
- [sparkles-core] range_ord_id is now starting from 0
- [sparkles-core] Move `counts_per_ns` to separate header with encoder format version
- [sparkles-macro] New: Add `range_event_end!` macro
- [sparkles] New: `RangeStartGuard` is now can be used with `range_event_end!` macro
- [sparkles] New: Two init options: `init` and `init_default`
- [sparkles] Send 0x00 packet with timestamp frequency at the beginning of the stream

## [0.1.2] - 2024-09-20

Baseline version of the project.

### Features
- Instant and Range events are supported.
- Events are streamed to receiving app over TCP.
- Events are saved to JSON file (Perfetto format).