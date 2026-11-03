# Mod support investigation

Reviewed 2026-11-04 against the pinned game used by RavenDroid 0.20/0.21. Mod support is **not enabled or validated** by the input/resolution update.

## Confirmed in the supplied game

The verified UnityWebData container contains `Il2CppData/Metadata/global-metadata.dat` (7,477,152 bytes, IL2CPP metadata version 27). The `data.unity3d` header identifies Unity **2020.3.49f1**. Read-only metadata inspection found `ModManager`, `LoadAllModContent`, `LoadSingleModContentBundle`, `LoadModWorker`, `AdditiveLoadSingleMod`, `RavenscriptManager` and MoonSharp interpreter names. Workshop/Steamworks metadata is also present.

These names demonstrate retained metadata, not that the code paths are reachable, working or supported in this WebGL build. A static string search cannot establish whether a platform guard disables loading or a method was stripped/stubbed. No mod was executed, no game code was patched and no Unity/tool pack download was needed for this investigation.

The game's MODS screen was also opened successfully in the Android emulator using a virtual USB mouse. It explicitly reported that Ravenfield was not connected to Steam and no Steam Workshop mods were available. The observed screen offered Browse Workshop; it did not expose a local-file importer. A virtual USB gamepad moved the menu cursor and selected Back successfully. This establishes that the mods UI is reachable, not that a mod content loader works. The session remained free of GL/shader failures.

## Current host limitations

- The virtual filesystem exposes the reviewed, immutable UnityWebData contents and a separate save namespace. There is no bounded external mod directory mount/importer or mod-selection UI.
- The data importer requires the pinned whole-container hash. Adding files to that container is not an accepted mod-installation mechanism.
- The Android executable hosts a WebGL-built Unity player. It does not become an Android Unity player merely because the host is native. Windows/Android bundles cannot be assumed compatible with the embedded WebGL player.
- Steam Workshop integration has not been implemented. Metadata referring to it does not supply Steam connectivity.
- Arbitrary new compiled C# behavior cannot be assumed loadable by this IL2CPP/AOT player. Existing script bindings and asset types would have to support the requested mod. Lua/Ravenscript is a plausible research path because MoonSharp metadata exists; functional availability is unproven.

## Practical next experiment

1. Use one small mod with author permission and available source, targeting the matching Unity/Ravenfield API. Check whether its assets can be built for WebGL and whether it requires code stripped from the player.
2. Trace the game's local-mod discovery path and loading trigger. Establish required paths, format, initialization order and any platform guards before creating a generic mod browser.
3. Add only the required bounded, read-only mod mount/import path, preserving the base-game hash, save isolation, archive traversal checks and import budgets.
4. Test one asset bundle and, separately, one minimal Ravenscript behavior. Record load failures, rendering, memory and script API coverage. Keep incompatible mods isolated; do not claim Workshop-wide support.

## Primary references

- [Official Ravenfield modding instructions](https://ravenfieldgame.com/modding.html): Unity 2020.3 and the Ravenfield Tools Pack; tool/game compatibility matters.
- [Official Ravenscript documentation](https://ravenfieldgame.com/ravenscript/): Lua-based scripting and exposed game/Unity APIs.
- [Unity AssetBundle building documentation](https://docs.unity3d.com/2019.4/Documentation/Manual/AssetBundles-Building.html): bundles are built for a target platform.
- [Unity AssetBundle platform considerations](https://docs.unity.com/en-us/engine/6000.0/manual/assets-and-media/assets-managing-runtime/assetbundles-section/platforms): platform-specific bundle restrictions. This newer manual supports the general platform constraint, not a compatibility guarantee for the pinned 2020.3 player.

The private read-only inspection summary remains in ignored `artifacts/mod-support-021-research.json`; no extracted game metadata is checked into source.
