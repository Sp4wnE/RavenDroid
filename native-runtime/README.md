# Native runtime

Rust host for the reviewed Unity WebAssembly build, using Wasmtime49, GLES/EGL, a virtual filesystem and native audio. The Android runtime excludes the AOT compiler and accepts only trusted matching artifacts.

See [architecture](../docs/ARCHITECTURE.md), [building](../docs/BUILDING.md) and [validation](../docs/VALIDATION.md). Cargo.lock and the static ABI/build profiles are retained for reproducible development; personal device/session reports are not part of the repository.
