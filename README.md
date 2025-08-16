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


## ✧ Requirements
🌟 STD support (general accuracy)  
🌟 `x86`/`x86_64`/`aarch64` architecture. (CPU cycle precision)

## ✧ Benches
Single event overhead on average x86 machine (Intel i5-12400) is ~9ns.


## ✧ Implementation status
Currently, you cannot attach additional data to your events (they are differentiated only by string name).
Supporting additional attached data is important feature, but it will take some time to find smooth and performant way to implement it.

Primary focus now - is a dedicated app for real-time event simultaneous streaming from multiple applications. It will also allow to remove dependency from protobuf protocol libraries.

## ✧ Known issues and limitations
✧ Converting timestamp to nanosecond time only have local consistency. Long sessions (day and more) can go out of sync with system clock.  
✧ Currently can have only 256 unique event names per thread  
✧ Currently up to 256 opened but not closed ranges at a time are supported (mostly enough)  
✧ No std support for now  
✧ Cannot specify UDP address to listen on  
✧ Proper usage of sparkles-macro::range_event_start!("name") emits warning  
✧ Timestamp wrap-around is not handled well (not an issue for 64-bit systems)  
✧ Naive handling for bad network conditions: if at least one packet lost, the whole tracing data packet is dropped.  
✧ Congestion control is not implemented for UDP streaming


## ✧ Crate features
✧ **accurate-timestamps-x86** - Enable serialization for x86/x86_64 timestamps. Trade off timestamp accuracy for higher overhead (slightly).  
✧ **self-tracing** - Add global buffer flushing events  
✧ **udp-streaming** - Real-time event streaming via UDP

｡ﾟﾟ･｡･ﾟﾟ｡  
ﾟ。SkyGrel19 ✨  
　ﾟ･｡･
