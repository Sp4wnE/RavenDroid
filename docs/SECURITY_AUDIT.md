# Security, privacy and repository audit

Audit date: 2026-11-01. Baseline: `19cb6181e0f30223eed0cbf2657b840b928efee3`, tag `v0.20`. This is a point-in-time source/artifact audit, not a penetration test or a malware-free guarantee. Sensitive values are deliberately omitted or masked.

The user's remote deletions of `AGENTS.md` and `docs/PRIVACY.md`, and README edits, were fast-forwarded before inspection. Those files have not been restored. No history rewrite, file removal, release removal, visibility change or engine upgrade is authorized by this report.

## Coverage and reproducibility

- Scanned all 174 tracked files, 180 unique reachable historical blobs and all seven reachable commits, including author and committer headers. Focused manual review covered manifests, importer/AOT trust boundaries, virtual filesystem, diagnostics, scripts, CI and documentation; this is not a line-by-line proof of every runtime implementation. Remote enumeration found one branch (`main`) and one tag (`v0.20`). Deleted documents were examined only as historical objects.
- Gitleaks 8.30.1, downloaded from its official release and SHA-256 verified: Git `--all --full-history`, Git `--all --reflog`, and an isolated tracked-file snapshot with archive/decode depth 2. All three scans returned exit 0 and zero secret findings. Gitleaks reported five patch-bearing commits; the independent header enumeration covered all seven commits. Gitleaks does not identify every kind of personal information.
- Additional masked scanning covered email patterns, workstation home paths, known owner identifiers, private-key headers, token shapes and literal physical ADB serial selectors. License matches and virtual filesystem paths were manually classified.
- Examined all five tracked binaries: three PNGs, Gradle wrapper JAR and a 66-byte smoke WASM. The wrapper JAR matches Gradle's official 8.13 wrapper checksum. All PNG chunk lists were parsed; the gameplay screenshot was also visually reviewed.
- Examined all 32 entries of the released APK's local byte-identical copy, native ELF sections, APK signer/manifest, and the entire decoded 1,043,179,318-byte game container. Additional UTF-16LE/BE scans found zero known-owner/home-prefix matches in those entries and the decoded container. The server asset digest matches local SHA-256 `3a6aaf1598b674c53b12fe6a662b808751bae0d640f2c8f4dc673cf8dbbd422a`, size 1,020,492,173 bytes. An additional network download of the same 1 GB file was unnecessary.
- Queried OSV for 167 registry packages in Cargo.lock and three explicitly versioned Maven dependencies. This is not a resolved Gradle/Python/system dependency inventory. Only Wasmtime matched; 24 database records reduce to 22 distinct advisories after alias deduplication.
- Read GitHub repository settings, refs, releases, Actions permissions, workflow runs and artifacts. Actions is disabled; zero runs and zero artifacts were returned. There are consequently no existing Actions artifact archives/logs to inspect. One release contains the APK and its checksum.
- Local evidence is kept under ignored `artifacts/security-*`; it is not committed. Only the specifically named local history backup's author headers were read to recognize prior owner markers. Private archives, SDKs, build caches and unrelated files were not swept.

## Findings, ordered by severity

### S01 — Critical upstream advisory match; application exploitability unproven

**Location:** `native-runtime/Cargo.toml:70-71`, `native-runtime/Cargo.lock:1275-1276`; trusted native load at `native-runtime/src/lib.rs:706`.

Wasmtime is pinned to 49.0.0. The upstream ARM64 Cranelift advisory [GHSA-mghf-wjfp-p5xm](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-mghf-wjfp-p5xm) describes a sandbox escape involving an exceptionally large `br_table`; 49.0.4 fixes that advisory. Later matched advisories require 49.0.5 on the same release line. The confirmed finding is the affected **version**, not a demonstrated exploit in this game.

The production host excludes the compiler, verifies the packaged AOT profile/hash, does not accept user-supplied native code and uses a pinned game. Those restrictions reduce input exposure; they do not prove that the precompiled game cannot contain an affected instruction sequence. Component-model advisories are not automatically applicable to the production core-module profile. Guest exception/GC support warrants separate review. The appendix lists every match and its initial applicability classification.

**Remedy:** plan a coordinated Wasmtime/Cranelift update to a reviewed patched version (at least 49.0.5 for this snapshot's matched advisories), regenerate matching AOT and dependency notices, and run runtime/GPU/Android regression checks. Do not swap only Cargo.toml or reuse incompatible serialized AOT. No motor/profile change was made during this audit.

### S02 — High distribution risk: public release contains the game

**Location:** `README.md:18`, `README.md:26`, `tools/source-lock.json:2-3`; binary `Releases/v0.20/ravendroid-v0.20-full.apk` → `assets/bundled/manifest.json` and `assets/native/unity.cwasm.bin` (binary: no source line).

The repository is now **public**, despite the earlier private setup. The published APK includes game data and compiled game code; logos and a gameplay image are also tracked. The pinned third-party source mirror has an MIT notice in its LICENSE. That notice alone does not establish that the mirror's author owns, or may sublicense, all Ravenfield/Unity artwork, audio and game code. The [official game listing](https://store.steampowered.com/app/636480/Ravenfield/) identifies SteelRaven7 as developer/publisher. No rightsholder redistribution authorization was found among tracked files. This is a confirmed evidence gap and distribution exposure, **not a legal determination of infringement**.

**Remedy:** verify and retain applicable rightsholder permissions separately; consider source/runtime-only distribution and user-supplied authorized game data if permission cannot be established. Public/private access and release removal are owner decisions; neither was changed automatically. Dependency license notices were retained.

### S03 — High release-hardening gap: debuggable APK and Android Debug signer

**Location:** `tools/build-phone-apk.ps1:13`; APK compiled `AndroidManifest.xml` and signing block (binary: no source line).

`aapt dump badging` reports `application-debuggable`. `apksigner verify --verbose --print-certs` verifies the APK's v2 signature and reports the generic Android Debug certificate. This is not an owner-specific certificate identity, and no signing private key is tracked. A debuggable application enables development access on an appropriately authorized device; the risk is conditional, not unauthenticated Internet access. The native app requests no INTERNET permission. Sharing a checksum in the same release does not independently authenticate the publisher against account compromise.

**Remedy:** prepare a non-debuggable release variant and a controlled release-signing/update-key migration plan. Replacing the signer casually can prevent updates over installed copies and jeopardize saves if users uninstall. The existing APK/signature was not changed.

### S04 — Medium privacy: six web-created commits identify the GitHub account

**Location:** commit-object `author` header (line 1 of metadata, not a source file) in `f467125`, `a5de057`, `cef981d`, `39ea345`, `ed28c8f`, `19cb618`.

Each of these six commits contains an account-associated address of the form `[REDACTED]+[REDACTED]@users.noreply.github.com`; the committer is GitHub's generic service identity. The address is a GitHub no-reply alias, not a personal mailbox, but exposes account linkage and a numeric account identifier. The original root commit uses a generic project identity. The known former personal mailbox was not found in any current reachable commit or source blob. The old pre-cleanup commit endpoint again returned no matching commit in this repository.

**Initial recommendation:** project-local Git identity is already generic; GitHub web edits use the account's web identity independently. Removing the existing author aliases requires an explicitly approved, narrowly scoped history rewrite and force push. A `.mailmap` would not erase raw commit headers and would itself store the original alias, so none was added.

**Subsequent owner-approved remediation:** after reviewing the report, the owner explicitly approved rewriting only these six commits' identities. Their author and committer names/emails were replaced with generic project identities; trees, messages, dates and user deletions were verified unchanged. GitHub signatures necessarily ceased to apply and were removed. A force-with-lease constrained to the observed old main moved it to `7789ebf7d147c8d54fdb80e99e07f134c11f2d9c`. All seven active remote commits now have generic project identities. The tag, release ID and both release asset digests are unchanged. The local report/cleanup commits were reparented onto that cleaned history and remain unpushed.

**Still unresolved:** direct GitHub API lookup of the old tip `19cb6181e0f30223eed0cbf2657b840b928efee3` still returns its account-associated author alias. Thus the active branch is clean, but old objects are not proven erased. Request GitHub Support review of retained objects/cached references, or discuss a separately authorized repository replacement if needed; a no-reply alias may not qualify for provider sensitive-data purging. Existing clones/downloads cannot be recalled. No repository deletion or cache-erasure guarantee is implied.

### S05 — Medium preventive control gap: secret protection is disabled

**Location:** GitHub repository settings `security_and_analysis` (API setting: no file/line).

Secret scanning, push protection and Dependabot security updates are disabled. Actions is also disabled. No leaked credential was detected by the local scans; disabled prevention is not evidence of compromise.

**Remedy:** review and enable the desired repository-side protections with the owner. Keep local Gitleaks checks even when host-side controls are available. No GitHub setting was changed by the audit.

### S06 — Medium supply-chain hardening: mutable Action refs and missing wrapper checksum

**Location:** `.github/workflows/checks.yml:12,13,17,19,21,27`; `gradle/wrapper/gradle-wrapper.properties:3`.

Six Actions are referenced through movable major-version tags. The wrapper uses HTTPS and validates the distribution URL, but has no `distributionSha256Sum`. The workflow has read-only repository permissions and uses `pull_request`, not `pull_request_target`; no explicit secret interpolation was found. Actions is currently disabled, so this is a latent hardening gap, not evidence of an executed malicious workflow. Python's Playwright range also floats and the full transitive toolchain was not reproduced.

**Remedy:** pin Actions to reviewed full commit IDs, disable checkout credential persistence, and pin the official Gradle distribution checksum. Keep dependency updates deliberate. These are suitable for the separate safe-cleanup commit; they require no game rebuild or history changes.

### S07 — Medium potential future privacy regression in alternate build scripts

**Location:** `native-runtime/scripts/build-android.ps1:29`, `native-runtime/scripts/build-android.sh:10`; compare `tools/build-android-jni.ps1:20`.

The current phone JNI build helper remaps the workspace prefix. The two older alternate entry points do not apply equivalent remapping. The current distributed JNI has no identified owner workstation path, no `.debug*` sections and no static `.symtab`; this finding concerns **future builds through different entry points**, not a demonstrated leak in the released JNI. Remapping only the workspace is also not a general guarantee for every compiler/cache/tool path.

**Remedy:** consolidate or consistently apply compiler-path privacy settings in a separately tested build change, including Cargo/tool roots where appropriate. Preserve existing ABI/features/profile checks. Do not regenerate the 1 GB release just to change documentation.

### S08 — Low cleanup candidate: legacy prototype remains connected

**Location:** `settings.gradle:9`, `app/build.gradle:5-11`, `.github/workflows/checks.yml:20,26,31`, `tests/test_input.py:25`, `tools/restore_handoff_assets.py:16`.

The WebView prototype is a separate Gradle module, has its own Java/JS input tests, and is still referenced by the workflow and developer helpers. The handoff restore tool is pinned to an old 0.4 APK and validates hashes. These are historical development paths, but they are not proven globally unused. Blindly removing `app/`, `tests/` or restore scripts would break those references or discard a fallback. No generated APK/AOT payload, physical ADB transcript or large session log is tracked.

**Remedy:** prepare a separate retirement plan enumerating module, CI, documentation and test dependencies; remove only after owner approval. No tracked files were deleted or moved.

### S09 — Low future-accident risk: ignored-file protection is incomplete

**Location:** `.gitignore:1-39`.

Known generated directories, keystores and `.env` files are excluded, but APK/AAB files, raw serialized AOT files, standalone private-key containers and debug logs copied to other locations are not generally excluded. Ignore rules do not untrack files or scrub history, and `git add -f` bypasses them.

**Remedy:** add narrow preventative patterns for generated distribution/native payloads and credential containers, without excluding required smoke WASM or dependency notices. This is suitable for the separate safe-cleanup commit.

### S10 — Low privacy consideration: support reports are user-shareable diagnostics

**Location:** `native-android/app/src/main/java/dev/ravenfield/nativehost/MainActivity.java:100,234-255`; `NativeCrashSummary.java:106-123` in the same directory.

Runtime diagnostics include manufacturer/model, Android version/ABI and selected native crash fields, including frame paths. The inspected code copies reports on explicit user action; no automatic network upload was found. No literal physical serial/Android-ID value was found in tracked sources. This is expected diagnostic behavior, not a confirmed current repository leak.

**Remedy:** review reports before sharing and optionally add redaction/export controls in a separately tested UI change. Do not replace user control or strip useful crash evidence during this repository-only cleanup.

## Binary and personal-data classification

- The three tracked PNGs contain no EXIF, textual, C2PA/provenance or timestamp metadata chunks. The gameplay screenshot shows gameplay, not personal desktop/phone UI. APK image resources match the inspected originals.
- `native-runtime/src/environment.rs:59` contains the fixed virtual guest home `/home/web_user`. The same constant appears in JNI at byte offset 372050. It is not the developer's home directory and must not be rewritten as a privacy fix.
- The AOT contains 41 ASCII home-path-shaped strings beginning at byte offset 327178125, such as `/Users/[REDACTED]/build/output/unity/...`. These are embedded upstream Unity/PhysX build/assertion strings. None matched the identified owner paths/mailbox. They remain upstream build traces; patching serialized native/game bytes would invalidate hashes and may break execution. No claim that the binary contains zero path-shaped strings is made.
- JNI retains a compiler `.comment` section. Compiler/version information is a build fingerprint, not an identified personal secret. The generic Android Debug certificate is likewise not owner PII.
- Email matches in `native-android/app/src/main/assets/THIRD-PARTY-NOTICES.txt:2249,2485` belong to dependency copyright/license notices. They were not removed. The APK includes those notices as well.
- Eight short owner-marker matches occur inside decoded binary game data; their surrounding byte patterns were inspected, including repeated mesh/index sequences. They do not establish an embedded owner identity. Random compressed/mesh bytes also resemble email addresses. A regex hit alone is not evidence of a person's email or a secret. The decoded game hash remains `57e509e2efd374f86df556c539236c3b7a1a6adae5eaf01dd5388688492c4a5c`.

## Why deleting a file is insufficient

Commit author/committer headers and previous file versions are immutable objects. A normal deletion commit leaves their contents reachable through earlier commits and tags; release assets, Actions artifacts, clones, forks and caches are separate copies. For real credentials, revoke/rotate first. A coordinated rewrite must cover affected refs and account for old clones reintroducing data; some cached references require GitHub Support. Provider backups and copies already downloaded cannot be promised erased. See [GitHub's sensitive-data removal guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository).

The retained local pre-cleanup backup is outside the active repository. It still contains historical personal information by design; it was not uploaded. This audit does not delete that backup or scan unrelated personal data.

## Boundaries and separate remediation

No executable exploit was run, no unknown AOT was deserialized, no emulator/phone was used, and no heavy rebuild was started. A zero-result secret scan does not prove absence of all secrets or encoded personal data. Opaque compressed/media payloads and original game metadata cannot be exhaustively interpreted by regex. Dependency advisories are version matches until profile/reachability is established; complete transitive Gradle/Python/Android-driver vulnerability coverage remains unverified.

Public visibility is now a verified current fact. Historical private-only statements in deleted documents reflected the earlier state; they were not restored or silently imposed on the owner's changed repository. No automatic attempt was made to restore privacy or delete releases. The six-commit identity rewrite happened only after the owner's separate explicit approval recorded above.

## Safe-cleanup status

Report committed before cleanup. A separate local cleanup commit addresses S06 by pinning the six Actions to the full commit IDs resolved from their existing version tags, disabling checkout credential persistence, setting a 30-minute job timeout and adding the official Gradle distribution checksum. It addresses S09 with ignore guards for generated APK/AAB/AOT/native payloads, logs and private-key containers. The root smoke WASM, license notices and user deletions are preserved. Local YAML/ignore/checksum checks and a final redacted Gitleaks scan verify these configuration changes; Actions remains disabled, so no hosted workflow execution is claimed. Engine changes, signer migration, history rewriting, file retirement, protection settings and game redistribution decisions remain outside that cleanup commit. No audit/cleanup commit has been pushed automatically.

## Matched dependency advisories

All rows locate to `native-runtime/Cargo.toml:70` and `native-runtime/Cargo.lock:1275-1276`. Version match is verified; applicability labels describe static configuration evidence, not exploit tests. Each linked upstream record describes its own severity; S01 conservatively carries the critical ARM64 match. The 49.x fix column is advisory-specific, not a complete upgrade prescription.

| Advisory | Summary | 49.x fix | Production-profile assessment |
| --- | --- | --- | --- |
| [RUSTSEC-2026-0315](https://rustsec.org/advisories/RUSTSEC-2026-0315.html) | call_ref and exception catch can drop some fuel accounting, leading to exponential fuel amplification | 49.0.1 | Fuel disabled in shipping epoch profile; other fuel profiles require review. |
| [RUSTSEC-2026-0316](https://rustsec.org/advisories/RUSTSEC-2026-0316.html) | Dynamic record lifting can allocate beyond the hostcall fuel limit | 49.0.1 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0325](https://rustsec.org/advisories/RUSTSEC-2026-0325.html) | Mis-typed WebAssembly tag imports can lead to GC heap corruption | 49.0.2 | Exception/GC boundary review needed; EH enabled, Wasm GC/function references disabled. |
| [RUSTSEC-2026-0326](https://rustsec.org/advisories/RUSTSEC-2026-0326.html) | Rooting for GC values live across try_call may be missing, causing GC heap corruption | 49.0.2 | Exception/GC boundary review needed; EH enabled, Wasm GC/function references disabled. |
| [RUSTSEC-2026-0327](https://rustsec.org/advisories/RUSTSEC-2026-0327.html) | Wasmtime component async-lifted callback result count is unvalidated, causing a native stack buffer overflow | 49.0.2 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0353](https://rustsec.org/advisories/RUSTSEC-2026-0353.html) | Panic when invoking an async-lifted function via call | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0354](https://rustsec.org/advisories/RUSTSEC-2026-0354.html) | Host panic when component call_async is cancelled under some conditions | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0355](https://rustsec.org/advisories/RUSTSEC-2026-0355.html) | Panic viewing component model stream or future as the wrong type | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0356](https://rustsec.org/advisories/RUSTSEC-2026-0356.html) | Panic on erroneous call-to-host from guest realloc call | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0357](https://rustsec.org/advisories/RUSTSEC-2026-0357.html) | Out-of-bounds zero-length stream writes can panic | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0358](https://rustsec.org/advisories/RUSTSEC-2026-0358.html) | Yielding in a component start function can lead to a host panic | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0359](https://rustsec.org/advisories/RUSTSEC-2026-0359.html) | Stack overflow dropping a long chain of recursive types | 49.0.3 | Recursive-type drop condition not tested; arbitrary Wasm is not accepted. |
| [RUSTSEC-2026-0360](https://rustsec.org/advisories/RUSTSEC-2026-0360.html) | Overlapping string transcode in a fused adapter can panic | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0361](https://rustsec.org/advisories/RUSTSEC-2026-0361.html) | Panic when pending calls are run after a trap | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0362](https://rustsec.org/advisories/RUSTSEC-2026-0362.html) | Wasmtime may not properly enforce async-entrypoint requirement | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0363](https://rustsec.org/advisories/RUSTSEC-2026-0363.html) | Component-model handle tables are capped only per instance and charged to no limit | 49.0.3 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0364](https://rustsec.org/advisories/RUSTSEC-2026-0364.html) | Exception tags can cause host data structures to be unaligned | 49.0.3 | Exception/GC boundary review needed; EH enabled, Wasm GC/function references disabled. |
| [RUSTSEC-2026-0378](https://rustsec.org/advisories/RUSTSEC-2026-0378.html) | Panic lowering a host-created component model stream into a guest that expects a different payload type | 49.0.4 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
| [RUSTSEC-2026-0379](https://rustsec.org/advisories/RUSTSEC-2026-0379.html) | Cranelift backend on aarch64 can jump to incorrect target on large Wasm br_table instruction | 49.0.4 | ARM64 AOT compiler relevant; pinned game trigger not established (critical upstream). |
| [RUSTSEC-2026-0422](https://rustsec.org/advisories/RUSTSEC-2026-0422.html) | Typed functions with i31 or nullref arguments may panic due to insufficient type-check | 49.0.5 | GC typed/host-created type conditions not established; Wasm GC disabled. |
| [RUSTSEC-2026-0423](https://rustsec.org/advisories/RUSTSEC-2026-0423.html) | Cross-engine type confusion with host-created struct-types-with-supertypes | 49.0.5 | GC typed/host-created type conditions not established; Wasm GC disabled. |
| [RUSTSEC-2026-0424](https://rustsec.org/advisories/RUSTSEC-2026-0424.html) | Unchecked memory usage in transferring borrowed resources between components | 49.0.5 | Component-model/async feature path excluded from shipping profile; developer python-cache differs. |
