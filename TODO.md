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
🌟 Better timestamp speed interpolation in parser  
🌟 Sparkles-parser: read and parse in separate threads  
🌟 Multicast discovery support

TODO:  
⚙️ Add support for `Send` ranges (start and end in separate threads)
⚙️ Congestion control for UDP streaming  
⚙️ Track changes in structs encoded/decoded by `bincode`  
⚙️ Include git revision into build  
⚙️ Option to run without an additional bg thread  
⚙️ Defmt support  
⚙️ Additional attached binary data  
⚙️ Option to limit total consumed TLS buffer allocation  
⚙️ Module info support: full module path, line of code  
⚙️ Async support  
⚙️ NO_STD implementation  
⚙️ tags / hierarchy of events  
⚙️ Viewer app  
⚙️ Multi-app sync  
⚙️ Global ranges  
⚙️ Measurement overhead self-test
