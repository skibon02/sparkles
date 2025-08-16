# Change Log
All notable changes to this project will be documented in this file.

The format is partially based on [Keep a Changelog](http://keepachangelog.com/)

## Version numbering
For `sparkles` and `sparkles-parser` version number is adjusted without strict rules or guarantees.  
`sparkles-core` have the same version number as `sparkles`.  
For protocol used by both crates there are some guarantees based on version number (Not applied to discovery protocol):
- When major proto version is different for encoder and decoder, correct parsing is not guaranteed.
- When major proto version is the same, but decoder's minor version is lower than encoder's, correct parsing is not guaranteed.
- When major proto version is the same, but decoder's minor version is higher or equal than encoder's, correct parsing is guaranteed.

### [Unreleased] sparkles
- Add `unstable-thread-id` feature for platforms without `thread-id` implementation (requires nightly)
- Fix panic on systems without process_name
- Config: rename `flush_threshold` to `sending_threshold`
- Config: add `auto_send_ms` for configuring maximum interval for sending trace data packets, improving responsiveness in real-time parsing scenarios.
- Re-export `sparkles-macro` crate from `sparkles`
Examples:
- Enable UDP multicast by default
- Add `cross-thread-ranges` example

### [Unreleased] sparkles-core
- Add RISC-V 32-bit architecture timestamp support
- Refactor target detection
- Add `force-fallback-impl` feature to use `Instant`-based implementation instead of arch-specific
- Add timeout flush trigger, improving responsiveness in real-time parsing scenarios.
- Config: add `auto_flush_ms` for configuring maximum interval for flush trigger.
- Range guard is now Send: You can start and end range event in different threads.
- Put more stability in `IdMappingState::insert_and_get_id`: return 255 in case of overflow instead of returning wrong ids.

### [Unreleased] sparkles-parser
- `SparklesParser` now will send arrays of parsed events to the callback instead of single event.
- Implement parsing for new cross-thread events type.

## Versions
### [0.1.8] sparkles (proto-1.0)
- Integrate `multicast-discovery-socket` for easy local network discovery
- Monitor network interfaces for dynamic joining multicast group
- Allow UDP receiving side reconnection (e.g. after address/port change)
- More stable recv handling in UDP sender
- Move `self-tracing` to `sparkles-core`

### [0.1.1] sparkles-parser (proto-1.0)
- Integrate `multicast-discovery-socket` for easy local network discovery
- implement seq_num wraparound case in UDP parsing, find better approach for UDP handling
- Interactively choose UDP client in `sparkles-udp-parse-and-save`

### [0.1.6] sparkles-macro
- Add `calc_hash!` macro to calculate hash of the event name at compile time

### [0.1.5, 0.1.6, 0.1.7] sparkles (proto-1.0) - 2025-03-09
- New protocol version: 1.0
- Config: Now have two options for the destination (file or directory).
- Config: Flush threshold is now specified in bytes (default 64K).
- Config: Add UDP sender configuration.
- Added some config parameters validation
- UDP support: `wait_client_connected()` can be used to suspend execution until UDP client is connected and ready to receive data.
- Init: add warning if sparkles was already initialized earlier (implicitly).
- Sender thread: Improve sleeping 
- Send timestamp frequency before any events
- Use `parking-lot` mutexes
- Examples: improve examples, add params

### [0.1.0] sparkles-parser (proto-1.0) - 2025-03-09
- New protocol version: 1.0
- Prepare library for creating custom parsers
- Unify structure to work the same with UDP and file sources
- Add "self-tracing" feature to analyze parser performance and packet receive timings
- Improve time calculation by using timestamp frequency interpolation
- Receive and parse packets in separate threads

### [proto-1.0]
- Define two flavors of protocol: One for byte stream with ordering and delivery guarantees (saving to file, sending over TCP...). The other is
  for a new possible configuration: UDP packets (without ordering or delivery guarantee, but with per-packet integrity).
- For UDP protocol server need to receive Subscribe packet from client. After this packet is received, server can begin sending trace packets to this client.
- Define packet type as a 32-byte pseudo-random pattern (sha256 of header name) so it can potentially be easier to locate when data is corrupted.
- Introduce x.x versioning in protocol version. Both encoder and decoder know protocol version it was compiled with.
- Remove `serde` dependency. Now pure `bincode` is used for packet encoding.
- Timestamp frequency: send timestamp together with timestamp frequency for better time interpolation in parser.
- Use big-endian


### [0.1.4] - 2024-09-28
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
- [protocol] Add ticks_per_sec packet type
- [sparkles-parser] Add small offset when several events recorded with the same timestamp


### [0.1.3] - 2024-09-23
- [sparkles-core] New: Add configuration structures: `SparklesConfig` and `LocalStorageConfig`
- [sparkles-core] Fix: Add distinction between events with the same name but different categories
- [sparkles-core] `ThreadInfo` is now a part of `LocalPacketHeader`
- [sparkles-core] range_ord_id is now starting from 0
- [sparkles-core] Move `counts_per_ns` to separate header with encoder format version
- [sparkles-macro] New: Add `range_event_end!` macro
- [sparkles] New: `RangeStartGuard` is now can be used with `range_event_end!` macro
- [sparkles] New: Two init options: `init` and `init_default`
- [sparkles] Send 0x00 packet with timestamp frequency at the beginning of the stream

### [0.1.2] - 2024-09-20

Baseline version of the project.

Features:
- Instant and Range events are supported.
- Events are streamed to receiving app over TCP.
- Events are saved to JSON file (Perfetto format).