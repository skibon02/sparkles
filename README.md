# .・゜゜ 𝕊ℙ𝔸ℝ𝕂𝕃𝔼𝕊 ・゜゜・
<img src="https://img.shields.io/crates/v/sparkles"></img>
<img src="https://img.shields.io/crates/size/sparkles"></img>

Performance-focused library for capturing execution flow of your application.

![img_1.png](https://github.com/skibon02/sparkles/blob/main/img_1.png?raw=true)

**What?**  
Simply add the instant_event! macro to your code with a string literal, and you'll be able to view this event later on a timeline with CPU cycle precision.  
**How?**  
Fast. Blazingly fast. 🚀 Recording a single event incurs an overhead as low as 12ns and consumes only 3 bytes in the trace buffer (in dense tracing conditions).

˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘  
Up to 🫸100_000_000🫷 events per second can be captured in a local environment with no data loss.  
༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚

## ✧ Subprojects
- **sparkles**: Ready-to-use library for capturing events and saving them to file in lightweight encoded format.
- **sparkles-gui**: dedicated GUI application for real-time connecting to multiple clients and viewing their events. 
- **sparkles-core**: Common functionality for `sparkles` and `sparkles-parser` crates.
- **sparkles-macro**: Macros for encoding event names into compact representation.
- **sparkles-parser**: General parsing library for creating custom parsers. `bin` part of this crate provides simple script to convert `trace/*.sprk` files into `trace.perf` file (Perfetto format).

## ✧ How to use (Perfetto format)
1. Add sparkles as a dependency to your project
```bash
cargo add sparkles 
```
2. Add some instant/range events to your code

```rust
use std::time::Duration;
use sparkles::{instant_event, range_event_start};

// Refer to sparkles/examples/how-to-use
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
cargo install sparkles-parser --features bin-deps # Only once
sparkles-parse-and-save
```
5. Go to https://ui.perfetto.dev and drag'n'drop resulting `trace.perf` file.
6. Observe the result:
![img.png](https://github.com/skibon02/sparkles/blob/main/img.png?raw=true)

## ✧ How to use (Sparkles GUI)
TODO

## ✧ Requirements
🌟 `x86`/`x86_64`/`aarch64` architecture. (CPU cycle precision)  
OR  
🌟 STD support (general accuracy)  

## ✧ Benches
Single event overhead on average x86 machine (Intel i5-12400) is ~9-12ns.


## ✧ Implementation status
Currently, you cannot attach additional data to your events (they are differentiated only by string name).
Supporting additional attached data is important feature, but it will take some time to find smooth and performant way to implement it.

Primary focus now - is a dedicated app for real-time event simultaneous streaming from multiple applications. It will also allow to remove dependency from protobuf protocol libraries.

## ✧ Known issues and limitations
✧ External events: 127 overlapping range events are supported per source
✧ External events: 255 unique event names per source
✧ External events: Missed events due to lack of time sync points are dropped
✧ Events: 256 unique event names per thread  
✧ Events: Up to 256 overlapping ranges at a time are supported
✧ Events: Timestamp wrap-around is not handled well for 32-bit systems
✧ UDP streaming: Realtime viewing events may feel laggy (~100ms intervals) because of waiting for time sync points
✧ UDP streaming: Cannot specify UDP address to listen on  
✧ UDP streaming: Naive handling for bad network conditions: if at least one packet lost, the whole tracing data packet is dropped.  
✧ UDP streaming: Congestion control is not implemented
✧ Proper usage of sparkles-macro::range_event_start!("name") emits warning  


## ✧ Crate features
✧ **accurate-timestamps-x86** - Enable serialization for x86/x86_64 timestamps. Small improvement accuracy for slightly higher overhead.
✧ **self-tracing** - Add internal events for flushing global buffer
✧ **udp-streaming** - Real-time event streaming via UDP
✧ **unstable-thread-id** - Remove dependency on `thread-id` crate. Requires nightly compiler.
✧ **force-fallback-impl** - Use `Instant`-based implementation. Use this if your architecture is not supported and automatic detection failed.

｡ﾟﾟ･｡･ﾟﾟ｡  
ﾟ。SkyGrel19 ✨  
　ﾟ･｡･
