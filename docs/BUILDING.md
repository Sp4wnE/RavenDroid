# Building and verification

## Prerequisites

Use JDK 17, Android SDK 35, NDK 28.2, Gradle 8.13 and the Rust toolchain required by Cargo.lock. Tool locations must be supplied through environment variables or script parameters; no individual workstation layout is assumed. Windows helpers support a separately prepared tools directory. Cache dependencies before using their offline build mode.

Game data, transformed WebAssembly, AOT payloads and JNI outputs are not checked into Git. A source-only checkout cannot package the full game until the reviewed inputs have been prepared. Source hashes and transform/profile checks are deliberately fail-closed; do not change them merely to accept a different game build.

## Native library and AOT

Build the Android library with `tools/build-android-jni.ps1`, selecting ARM64 and `guest-eh-epoch-v1`. Supply `-JavaHome` explicitly. The helper remaps builder paths before compiling. The corresponding AOT must use the same engine configuration. Existing trusted, matching AOT should be reused.

The required static build inputs are `native-runtime/build-profile.json`, `abi-inventory.json` and `invoke-specializations.json`. Preparation and transformation helpers are under tools/ and native-runtime/examples/.

## Full APK

1. Prepare the reviewed raw game data and the matching AOT/JNI assets.
2. Run `python tools/prepare_bundled_data.py` to verify and prepare the bundled stream.
3. Run `tools/build-phone-apk.ps1 -JavaHome $env:JAVA_HOME -Gradle <gradle-command> -BundledData`.
4. Verify with `tools/verify_native_apk.py APK --bundled`, Android SDK apksigner and zipalign.

The default runtime-only package remains available by omitting `-BundledData`; it requires a separate data import. The emulator package uses a separate ABI, application ID and test-only driver wrapper.

Do not rename compressed parts to `.gz`: Android's asset merger treats that suffix specially and tries to decompress them. Opaque `.chunk` assets are stored without a second compression layer. Package hashes are listed in each release's `.sha256` attachment.

## Tests and resources

Java unit tests run through `:nativeApp:testDebugUnitTest`; lint through `:nativeApp:lintDebug`. Rust tests use native-runtime/Cargo.toml. GPU tests require the appropriate graphics environment and should run serially. BundledDataProbe.java can exercise fresh preparation and reuse directly from a built APK with a 256 MiB Java heap, using the production importer/stream classes.

Run heavy operations serially, use at most two workers, stop the emulator during heavy builds and monitor RAM/commit/disk. `run-memory-bounded.py` limits its child process tree without changing system pagefile settings.
