## ✧ Known issues
- Perfetto format export: incorrect start/end pairing for overlapping ranges with the same name (cross-thread-ranges example) - maybe not solvable by protobuf format

## ✧ Implementation status
Ready:  
🌟 Timestamp provider  
🌟 Event name hashing  
🌟 ~~Perfetto json format compatibility~~ (replaced with protobuf)  
🌟 Ranges (scopes) support  
🌟 Configuration support  
🌟 Perfetto protobuf format support  
🌟 Abstraction over events sending type (UDP/File)  
🌟 Automatic timestamp frequency detection  
🌟 aarch64 support  
🌟 More explicit and recoverable packets with known pattern  
🌟 Resistance to data loss during transmission  
🌟 UDP real-time reader and parser library API  
🌟 Better timestamp speed synchronization in parser  
🌟 Sparkles-parser: read and parse in separate threads  
🌟 Multicast discovery support
🌟 Add support for `Send` ranges (start and end in separate threads)
🌟 Viewer app (sparkles-gui)

TODO:  
⚙️ Add runtime name creation support
⚙️ Add tick information for last thread flush
⚙️ External event sources support (GPU events specifically)
⚙️ Ensure stability if a lot of threads are spawned over time (additional memory allocation for per-thread data)
⚙️ Include git revision into build  
⚙️ Multi-app sync  
⚙️ Global ranges  
⚙️ Async support (tokio task support)  
⚙️ Track changes in structs encoded/decoded by `bincode`  
⚙️ Option to run without additional bg thread  
⚙️ Defmt support  
⚙️ Additional attached binary data  
⚙️ Option to limit total consumed TLS buffer allocation  
⚙️ Module info support: full module path, line of code  
⚙️ NO_STD implementation  
⚙️ tags / hierarchy of events  
⚙️ Measurement overhead self-test
