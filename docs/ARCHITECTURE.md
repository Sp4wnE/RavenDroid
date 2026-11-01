# Architecture

RavenDroid hosts a pinned Unity WebGL/WebAssembly build using Wasmtime 49 and a custom native service bridge. It does not run the game in a WebView or compile the original Unity project.

## Runtime

- Java17 launcher, installer and touch UI; Android API 29 minimum, ARM64 production target.
- Trusted AOT code is produced on a development machine; the Android library excludes the compiler.
- One thread owns the VM and graphics context. UI/JNI operations enqueue input rather than sharing VM or GL handles.
- GLES 3/EGL presents to a SurfaceView. WebGL pointer bounds, robust access and driver error behavior are preserved.
- Symphonia decodes audio on the owner side; the AAudio callback mixes prepared audio without executing the VM or doing file I/O.
- Unity data is mounted through a disk-backed virtual filesystem; saves are separate from immutable game data.

## Full package preparation

The optional `ravenBundleData` build includes a gzip stream split into small opaque `.chunk` assets. BundledDataStream opens one asset at a time. GameDataImporter checks size and SHA-256, writes a partial file, then atomically replaces the target. Preparations are serialized and verified existing data is reused.

## Engine profiles

The current engine uses `guest-eh-epoch-v1`: guest exception boundaries and a 10 ms epoch watchdog, with a 60 s startup and 30 s callback deadline. Serialized AOT, manifest and exported JNI profile must match. Legacy fuel profiles remain separate. Never load arbitrary downloaded AOT artifacts.

Native optimization work includes typed GL imports, conservative state caches, deadline pacing and compressed-texture capability detection from actual supported formats. The full package reuses that engine. Experimental emulator presentation settings are not enabled in the phone build.
