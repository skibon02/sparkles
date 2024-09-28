# ✧ sparkles-core ✧
Core crate for [sparkles](https://crates.io/crates/sparkles)

## ✧ `no_std` support
Bare-metal systems are supported.
Alloc is required.

## ✧ Timestamp provedrs
Sparkles prefer to use timestamp directly from your CPU, so different timestamp providers are supported

- **x86/x86_64**: Comes in two variants: by default faster but not very accurate (+-4ns). 
If you need CPU cycle percicion, enable feature `accurate-timestamps-x86` (overhead is additional ~10ns)
- **aarch64**: Used on aarch64 CPU.
- **std**: Use `Instant::now`, which is slower, but should be supported by any other std environment.
- **cortex-m**: Requires feature `cortex-m`. Comes with additional `init()` method to enable cycle counter peripheral.

The appropriate implementation is selected at compile time depending on architecture and features.

