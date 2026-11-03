# Validation

## Recorded checks (0.20 baseline)

| Check | Result |
| --- | --- |
| Java unit tests | 34 passed |
| Android lint | 0 errors, 27 warnings |
| Full-size data preparation |Passed with256MiB Java heap |
| Existing data reuse |Passed without reading the bundled source again |
| Truncated/corrupt input |Rejected without replacing existing data |
| Concurrent preparation |Serialized to one writer |
| Asset stream |Lazy reads, empty/missing parts and close behavior tested |
| APK payload |AOT/JNI/profile/hash validation and full decoded game hash checked |
| Signing and alignment |Verified before release |
| Earlier native engine tests | 88 runtime and 7 GPU tests passed |

The full-data integration test executes the production Java importer and asset-stream logic on a development JVM. It is not an Android Activity or physical-device test. Earlier runtime/GPU results refer to the engine reused by this package; they are not a universal device guarantee.

## Remaining limitations

- The latest bundled first-launch UI still requires physical-device verification.
- Rendering coverage is limited to observed scenes. Other maps, materials and shadows may differ.
- No stable 60 FPS or across-device performance guarantee is made.
- Vehicle controls are manual; full driving/flying and all multi-touch combinations remain incompletely covered.
- Background/lock-screen session restoration is not an established feature.

Local session dumps, individual device reports, old handoffs and machine-specific build records are not included in this repository.

## 0.21 input, launcher and resolution checks (2026-11-04)

- 42 Java tests passed; Android lint: 0 errors / 27 warnings. Input tests cover repeat suppression, multiple device/modifier ownership, touch/hardware overlap, trigger release, disconnect/reset, rejected delivery recovery, dead zones, elapsed-time look integration and Android key/source mappings.
- The standalone Rust viewport test passed for all supported sizes, center/edge mapping and invalid-size rejection. This was a targeted test; the earlier full Rust/GPU suite was not repeated.
- ARM64 and x86_64 JNI builds passed with the existing engine/AOT profile. No full-game AOT rebuild or performance experiment was performed. Heavy jobs ran serially with a two-worker maximum and an enforced 8 GiB process-tree limit; the emulator was stopped during compilation.
- APK signing, size/hash/profile checks, 16 KiB ELF/ZIP alignment and the full decoded bundled-game hash passed. Game data and AOT hashes remain pinned.

| Emulator session | Verified render size | Frames | Coverage |
| --- | --- | --- | --- |
| Low resolution | 640×360 | 12,454 | Menus, Island/Battle/25+25, Deploy, keyboard movement/loadout, pause/return |
| Middle resolution | 960×540 | 1,114 | Fresh startup, gamepad mode/pause, graceful return |
| Default resolution | 1280×720 | 1,085 | Fresh startup, gamepad mode/pause, graceful return |
| Virtual USB HID | 1280×720 | 22,519 | Registered Android gamepad/mouse/keyboard devices; gamepad menu cursor/A selection, mouse MODS-menu selection, keyboard movement, gamepad right-stick look, mouse fire and captured relative motion, Start/pause |

All listed sessions ended with no first blocker, pending/consumed GL error or shader failure; held keys and pointer buttons were cleared. These mixed startup/menu/gameplay sessions are not controlled FPS benchmarks. Pointer capture was observed enabled on the RavenDroid window with `MOUSE_RELATIVE` events. ADB-only synthetic D-pad injection was insufficient to validate a device path, so actual Android virtual HID device registration was used.

On the final manifest, attaching and detaching a virtual USB keyboard preserved the same native-session start marker: frame 930 before attach, 1050 after attach and 1230 after detach. The final session then stopped cleanly. The English/Turkish notice, both notice buttons, saved resolution choice and question-button reopening were inspected through actual emulator UI; the GitHub target was checked in source. Opening an external browser was not exercised.

Physical phone, real controller-model and Bluetooth coverage remain unverified. Full aircraft/vehicle gameplay and every peripheral combination are not established. All virtual devices and the emulator were closed after testing. Private evidence remains outside tracked source.
