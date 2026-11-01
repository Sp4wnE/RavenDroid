# RavenDroid contributor instructions

- Preserve the existing architecture, working features, saves and control settings. Avoid unrelated refactoring.
- Read only files relevant to the task. Start with README.md and the relevant document in docs/; do not scan build/dependency directories or large local artifacts without a task-specific reason.
- Run necessary tests; avoid redundant broad test runs. Documentation-only changes do not require game/build tests.
- Run heavy builds serially with at most two workers and an 8 GiB process-tree limit. Confirm the emulator has stopped using host/ADB visibility; monitor free RAM, commit and disk. Do not change pagefile/security settings.
- Keep personal paths, device identifiers, session logs and local handoffs outside the repository. Use environment variables/parameters for tool locations and remap compiler paths in distributed binaries.
- Keep this repository private. Do not enable Pages or public sharing. ADB access is limited to this project.
- Local workspace memory may exist in the parent docs/ directory; never publish it. Use the concise English docs/ references for standalone checkouts.
