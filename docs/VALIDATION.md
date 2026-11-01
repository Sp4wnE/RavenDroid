# Validation

## Recorded checks

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
