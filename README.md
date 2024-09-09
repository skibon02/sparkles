# .・゜゜・ 𝕊ℙ𝔸ℝ𝕂𝕃𝔼𝕊 ・゜゜・．
<img src="https://img.shields.io/crates/v/sparkles"></img>
<img src="https://img.shields.io/crates/size/sparkles"></img>

Performance-focused library for capturing execution flow of application.

![img_1.png](https://github.com/skibon02/sparkles/blob/main/img_1.png?raw=true)
## Main parts
- **sparkles**: Ready-to-use library for capturing events and streaming them to receiving app over TCP
- **sparkles-core**: Common functionality for std and no_std version of sparkles.
- **sparkles-macro**: tracing_event! macro to encode event name into integer value.
- **sparkles-receiver**: This binary will listen to TCP port, capture and decode incoming events and save them to JSON file (Perfetto format).

## How to use
1. Add sparkles as a dependency to your project
```bash
cargo add sparkles 
cargo add sparkles-macro
```
2. Run receiving app in background
```bash
cd sparkles-receiver
cargo run --release --example listen_and_print
```
3. Add some events to your code

```rust
use std::time::Duration;
use sparkles_macro::tracing_event;

// Refer to sparkles/examples/how_to_use.rs
fn main() {
    let jh = std::thread::Builder::new().name(String::from("thread 2")).spawn(|| {
        for _ in 0..30 {
            tracing_event!("✨✨✨");
            std::thread::sleep(Duration::from_micros(1_000));
        }

        // It is required for now, will be replaced with drop guard in future
        sparkles::flush_thread_local();
    }).unwrap();

    for i in 0..1_000 {
        tracing_event!("✨");
        std::thread::sleep(Duration::from_micros(10));
    }

    jh.join().unwrap();

    // It is required for now, will be replaced with drop guard in future
    sparkles::finalize();
}
```
4. Run your code. As it finishes, trace.json is generated.
5. Go to https://ui.perfetto.dev and drag'n'drop resulting json file.
6. Observe the result:
![img.png](https://github.com/skibon02/sparkles/blob/main/img.png?raw=true)


## ✧ Requirements
🌟 STD support (works better on x86 architecture)

## ✧ Benches
Single event overhead on average x86 machine (Intel i5-12400) is 9ns.

˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘\
Up to 🫸100kk🫷 events can be captured in a local environment with no data loss. \
༘ ⋆｡˚ ༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚ ༘ ⋆｡˚༘ ⋆｡˚ ✧ ˚


## ✧ Implementation status
Ready: \
🌟 Timestamp provider \
🌟 Event name hashing \
🌟 Perfetto json format compatibility

TODO: \
⚙️ Ranges (scopes) support \
⚙️ Additional attached binary data \
⚙️ Module info support: full module path, line of code \
⚙️ Perfetto binary format support \
⚙️ Abstraction over events transfer type (TCP/UDP/IPC/File) \
⚙️ Capture and transfer loss detection with no corruption to other captured and transmitted data \
⚙️ Async support \
⚙️ Configuration support \
⚙️ NO_STD implementation \
⚙️ tags / hierarchy of events \
⚙️ Viewer app \
⚙️ Multi-app sync \
⚙️ Global ranges \
⚙️ Measurement overhead self-test

## ✧ Milestones
TODO

｡ﾟﾟ･｡･ﾟﾟ｡\
ﾟ。SkyGrel19 ✨\
　ﾟ･｡･
