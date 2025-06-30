# .・゜゜ 𝕊ℙ𝔸ℝ𝕂𝕃𝔼𝕊 ・゜゜・
<img src="https://img.shields.io/crates/v/sparkles"></img>
<img src="https://img.shields.io/crates/size/sparkles"></img>

Performance-focused library for capturing execution flow of your application.

![img_1.png](https://github.com/skibon02/sparkles/blob/main/img_1.png?raw=true)

**What?**  
Simply add the instant_event! macro to your code with a string literal and you'll be able to view this event later on a timeline with CPU cycle precision.  
**How?**  
Fast. Blazingly fast. 🚀 Recording a single event incurs an overhead as low as 10ns and consumes only 3 bytes in the trace buffer (in dense tracing conditions).

˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘  
Up to 🫸100_000_000🫷 events per second can be captured in a local environment with no data loss.  
༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚

## ✧ Main parts
- **sparkles**: Ready-to-use library for capturing events and saving them to file in lightweight encoded format.
- **sparkles-core**: Common functionality for std and no_std (todo) version of sparkles and protocol packets.
- **sparkles-macro**: instant_event! and range_event_start! macro to encode event name into integer value.
- **sparkles-parser**: Provides easy to use way of converting recorded trace data to Perfetto format as well as library for realtime parsing.

## ✧ How to use
1. Add sparkles as a dependency to your project
```bash
cargo add sparkles 
cargo add sparkles-macro
```
2. Add some instant/range events to your code

```rust
use std::time::Duration;
use sparkles_macro::{instant_event, range_event_start};

// Refer to sparkles/examples/how_to_use.rs
fn main() {
    let finalize_guard = sparkles::init_default();
    let g = range_event_start!("main()");

    let jh = std::thread::Builder::new().name(String::from("joined thread")).spawn(|| {
        for _ in 0..100 {
            instant_event!("^-^");
            std::thread::sleep(Duration::from_micros(1_000));
        }
    }).unwrap();
    
    std::thread::Builder::new().name(String::from("detached thread")).spawn(|| {
        for _ in 0..30 {
            instant_event!("*_*");
            std::thread::sleep(Duration::from_micros(1_000));
        }
    }).unwrap();

    for i in 0..1_000 {
        instant_event!("✨✨✨");
        std::thread::sleep(Duration::from_micros(10));
    }

    jh.join().unwrap();
}
```
3. Run your code. As it finishes, `trace/*.sprk` is generated.
4. Run `sparkles-parser` in the directory with `trace` folder.
```bash
cargo install sparkles-parser --feature --bin-deps # Only once
sparkles-parse-and-save
```
5. Go to https://ui.perfetto.dev and drag'n'drop resulting `trace.perf` file.
6. Observe the result:
![img.png](https://github.com/skibon02/sparkles/blob/main/img.png?raw=true)


## ✧ Requirements
🌟 STD support  
🌟 x86/x86_64/aarch64 architecture.  
**OR**  
🌟 Functioning `Instant::now()`

## ✧ Benches
Single event overhead on average x86 machine (Intel i5-12400) is 9ns.


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

TODO:  
⚙️ Track changes in structs encoded/decoded by `bincode`  
⚙️ Include git revision into build  
⚙️ Option to run without additional bg thread  
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

## Known issues and limitations
✧ Converting timestamp to nanosecond time only have local consistency. Long sessions (day and more) can go out of sync with system clock.  
✧ Currently can have only 256 unique event names per thread  
✧ Currently up to 256 opened but not closed ranges at a time are supported (mostly enough)  
✧ No std support for now  
✧ Naive handling for bad network conditions: if at least one packet lost, the whole tracing data packet is dropped.  
✧ Cannot specify UDP address to listen on  
✧ Proper using of sparkles-macro::range_event_start!("name") gives warning
✧ Timestamp wrap-around is not handled well (not an issue for 64-bit systems)
✧ Congestion control is not implemented


## Crate features
✧ **accurate-timestamps-x86** - Enable serialization for x86/x86_64 timestamps. Trade off timestamp accuracy for higher overhead (slightly).  
✧ **self-tracing** - Add global buffer flushing events

｡ﾟﾟ･｡･ﾟﾟ｡  
ﾟ。SkyGrel19 ✨  
　ﾟ･｡･
