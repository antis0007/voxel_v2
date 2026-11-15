# Canonical Axiom launch workflow

`tools/Play-Axiom.ps1` is the entrypoint. `Play-Axiom.cmd` is a convenience wrapper without execution-policy bypass. Launch never compiles, rebuilds, or selects binaries by timestamps. Build-Multiplayer.ps1 remains an explicit build command.

## Commands

- Normal play: `tools/Play-Axiom.ps1 -Mode Game -Profile Current`
- Validate target/hash without launching or creating save directories: add `-ValidateOnly`.
- Private QA writes: `tools/Play-Axiom.ps1 -IsolatedSaveQA`. A fresh directory is created below artifacts/launch-qa. User worlds/preferences can still appear in library discovery; use a new QA world only. Explicit output paths in forwarded game arguments are not rewritten.
- Low-spec or game flags: `tools/Play-Axiom.ps1 -LowSpec -GameArgs @('--flag=value with spaces')` in PowerShell. Arguments are never evaluated as script text. CMD callers follow normal Windows CMD quoting rules.
- Workshop relay host: `tools/Play-Axiom.ps1 -Mode Workshop -Name Player`
- Workshop relay join: add `-Role Join -Address SESSION-CODE`.
- Workshop headless server: add `-Role Server`.
- Workshop supports `-Transport Relay|Steamworks|Standalone`, `-Lobby`, `-LowSpec`, name sanitization, player/world data folders, exclusive host world lock and server.properties defaults. Steamworks requires its runtime DLL. `-Binary` requires an adjacent build.json with executable_sha256; unbound arbitrary executables fail closed.
- Direct multiplayer: `tools/Play-Axiom.ps1 -Mode Multiplayer -Transport Steamworks|Standalone -Role Host|Join|Server`. Address, AppId, ForceSdr, Seed, Generator and LowSpec retain their old meanings. Steamworks Join requires an explicit steam:ID. Native transports retain separate feature builds.
- Terrain review: `tools/Play-Axiom.ps1 -Mode RealTerrain` uses Current with --real-terrain and a fresh private write directory. Arbitrary old terrain binaries and the previously shared review save folder are no longer selected.

Normal Game mode leaves installed save/cache paths and inherited environment unchanged. QA is explicitly rejected for multiplayer modes rather than pretending to override their host/player save workflow.

## Ownership and integrity

`profiles.v1.json` maps each profile to its executable. Current points to
`target/shared-integration-4773c629/AXIOM.exe`, built from source commit
`4773c629` with the `release-fast` profile. `current-build.json` records
the source commit and executable SHA-256. The local executable lives under
the ignored `target` directory. Workshop uses its package's `build.json`.
The separate multiplayer profiles use `legacy-binary-hashes.json`.
The launcher checks the selected executable against its receipt before launch.

Core.ps1 owns profile/path/hash validation and the process boundary with cwd/environment restoration. Workshop.ps1 owns relay/session/data/locking behavior; Multiplayer.ps1 owns direct-transport flags and Steam environment. Neither module chooses a build. The injectable runner exists only in the internal process helper for no-UI tests; the public entrypoint cannot inject it.

Package-Workshop.ps1 copies this same entrypoint/modules and emits one package-local profile pointing to Workshop.exe/build.json. Existing distributed packages remain untouched. New package CMDs no longer bypass execution policy. Respect machine policy if execution is blocked.

## Retired names and recovery

Play-Workshop.ps1 and Play-Multiplayer.ps1 are thin parameter-compatible wrappers. Play-RealTerrain.ps1/.cmd delegate to the canonical terrain mode. Play-Integrated, Play-Playable, Play-Planet, Play-Manyfold, Play-HellGates and Play-OnlineReviewed now fail with an explicit retirement message; silently mapping historical scene/build names to unrelated new behavior would be misleading. Historical docs remain evidence, not current launch instructions.

Exact original script bytes, Package-Workshop.ps1 and checksums are backed up at D:\CodexArtifacts\axiom-generation-speed-20261110\launcher-overhaul-backup-20261111. The provisional release-launcher-20261111 PowerShell launcher now delegates to this entrypoint instead of owning a second selector; its old bytes are also backed up. No Desktop shortcut, user saves or existing binary was changed. No commit was made.

## Tests and remaining release gates

Run `powershell.exe -NoProfile -File tools/launch/Test-Launch.ps1`. Tests use inert .dat fixtures and an injected/mocked process boundary, never game UI, network sessions or Cargo. Fixtures remain in a uniquely named temp directory for inspection. Coverage: bad/missing hashes and malformed JSON, rooted and relative paths, argument boundaries, cwd/environment restore after success/failure, Workshop host/join data and protocol flags, direct server flags, and launcher parser checks.

V20 passed the release owner's scoped native acceptance: exact headless smoke, 443 focused tests, and a scripted new-game/gameplay/save/reload loop with data equality, post_reload_playable=true, capture_saved=true and stage-22-reloaded-world.png. The recorded run exited zero without timeout and used disposable saves; the release owner visually checked stage 22. Larger v71 and legacy v70 saved-scene reviews each ran for 15 seconds with background replacements. These samples do not prove every historical world, long-running stability, WAN play or a performance improvement; user worlds were not opened. Old native transport/package integrity is not a fresh gameplay acceptance claim.

Independent V20 PE inspection found only Windows system/API-set imports plus VCRUNTIME140.dll/UCRT, with no Steam/GNS imports or delay imports. VC runtime, UCRT, DXGI, D3D12 and Vulkan loader files exist on this computer. Compiler artifact features are default and multiplayer. Core assets and runtime shader inputs inspected are embedded; shader filesystem reads examined are build/test validation. GPU driver/backend availability on other computers is not certified. Repository cwd supports relative headless scenario and optional file paths; normal launch performs no hidden build or source-file loading for those embedded assets.

## Release selection history

2026-11-11 baseline: Current initially retained V16 and its native-build-v16/binary-hashes.json while later candidates were validated. V16 had recorded smoke/hash validation but was not accepted for the final native reload gate. Its artifact remains untouched.

2026-11-11 promotion: after explicit release-owner approval and independent receipt/hash checks, Current was atomically promoted to V20, release_id 2026-11-11-v20. The sole authoritative executable hash remains release-v20-validation-20261111/binary-hashes.json; no hash is duplicated here. Exact profile/README preimages and checksums are in D:\CodexArtifacts\axiom-generation-speed-20261110\launcher-v20-promotion-backup-20261111. Future candidate builds must not replace Current before passing their gates.
