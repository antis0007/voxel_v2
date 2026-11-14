# Ossuary Engine — 2026-10-26

## Inspected problems

- `crates/app/src/world/adventure/engine.rs`: the previous ram/sweep/volley loop changed spacing but never edited terrain. Manufacturing replaced modules, so damage primarily reduced one health pool.
- `crates/app/src/world/adventure.rs`: projectile collision used one 0.8 m body sphere for this boss; melee also selected the center. Its visual arm could not be deliberately disabled.
- `crates/app/src/world/adventure/reliquaries.rs`: reactivation constructed a fresh Engine. Only the Tollkeeper had an encounter snapshot, so Engine stock and health could reset on streaming or death.
- `crates/app/src/egui_ui/relics.rs`: the HUD discarded the supplied action state and displayed only Active/Enraged.

These are source findings. They do not establish that every player's previous strategy was circling and shooting.

## Implemented encounter

Existing kind 6, site/relic 3, item IDs, wand evaluator, factory power rules and relic progression remain in use. Activate the existing reliquary after clearing its guardians.

- Slow turn, then fixed direction throughout preparation, contact and recovery.
- Braced cutting advance, annular loading sweep (safe inside 1.5 m or outside 3.6 m), and finite-stock launcher when installed. The cutter and scoop have separate spatial responses.
- Two exposed side drives, each with 60 damage capacity. The loading drive stops manufacturing and sweeps; traction stops powered translation and steering. Failures never repair themselves.
- Frost lance/Cryo nova hitting a drive stalls the machinery for four seconds. Ordinary hits still damage the body. A heavy hit during transfer still detaches a manufactured module.
- Cutting uses admitted world edits and ordinary body collision. Weathered masonry, limestone, stone, timber and loose soils can be cut; basalt, reinforced construction, metals, ores and factory assemblies stop excavation. There is no trap escape or teleport fallback.
- Removed voxels become accounted cargo. With a broken loader they are dropped as ordinary stacks; a full drop pool retains overflow rather than discarding it. Completion preserves pending overflow in the save.
- Disable both drives, approach the rear control and press F to recover a Mining Drill. Destruction yields surviving completed modules and the original metal/power salvage. Claiming the relic still uses its existing progression path.
- Low ceramic cutting carriage, offset articulated loader, raised stone counterweight, silver drive fittings, narrow gold markings. Installed brace shutters visibly open during committed work; launcher and in-progress assemblies remain visible. Disabled drives and the collapsed loader change the silhouette.
- HUD now displays the supplied action state. Engine labels are short states rather than tactical narration.

## Persistence and generation

Save **38** adds an Engine section after the existing boss section: health, position, orientation, menace, drive damage, frost stall, finite stock, manufacturing/transfer/loose modules, excavation count and cargo. Active attacks cancel on restoration with two seconds of recovery. Completion continues to use the original one-time defeat bit. The factory kit mirror is updated with recovered hardware, preventing legacy reconciliation from removing the reward.

Versions 1–37 keep their old layout and remain readable. Those formats never stored partial Engine state, so an older awakened encounter initializes its mechanism state once. No invented partial damage can be recovered from those files. Older executables cannot read new saves.

Generator **59** uses the burial-works Foundry arena: weak burial partitions, hard basalt buttresses, open service aisles, rear galleries and displaced stone vessels. Older generator versions retain their previous arena sampler and cache key. Existing terrain is not regenerated.

## Work bounds

At most 384 front-face probes and 192 requested edits per Engine tick; 4,096 admitted excavation cells per encounter. The dirty-voxel queue must have room before cutting. Fine-scale faces complete across ticks. Cargo has one bin per cut material and at most three remaining finished modules. No dynamic lights or collision-bearing cosmetic debris were added. Performance at fine voxel scales remains unreviewed.

## Native evidence

Evidence is under `target/ossuary-review/`; each completed scripted run has a frozen executable, SHA-256 manifest, stdout/stderr, exit code and first-person PNGs.

- `final-Engineering`: real movement, Frost lance projectiles and the normal focused relic interaction disabled both drives and secured the cutter. At 7.51 s the Engine still had 178 HP with both drives disabled. The drill remained at exactly one through completion.
- Full region save/readback in that run: v38 restored health 334, loading damage 26, traction damage 0, stock 5, and two-second restoration recovery. A later full readback preserved the completed record and exactly one drill.
- `final-Cutting`: opened weathered masonry, advanced from z=18 to z=22.40 and retained exactly two removed cells as cargo. This is authoritative terrain/collision evidence, not proof of a human tactical decision.
- `final-Combat`: ordinary Spark projectiles completed destruction. No capture reward was granted. This run recorded a 482.152 ms maximum frame; its p99 was 4.328 ms. The spike is unresolved and prevents a clean performance acceptance claim.

These earlier runs precede the final coordination fixes: the arm cannot transfer and sweep together, frost pauses the action clock, and the sweep can commit before the carriage reaches exact facing. Automated outcomes are not manual feel acceptance.

### Final candidate and measurements

Final executable: `target/ossuary-review/final.exe`, SHA-256 `346A4789A6BD447C3F78B21911384C3738BEA0D261E32802697B7290950BC7D5`.

The earlier reviewed candidate is also preserved at `target/ossuary-review/candidate.exe` (SHA-256 `54523E8CCDA3466B4F238B575D30C662DD55119A22BF46BA1E6B896C7F18F815`). The final source removes a limit of 32 on the knock-off statistic: repeated recovery of the same physical module can legitimately exceed that statistic without creating resources.

Concurrent GPU-backend work temporarily broke an import, then corrected it. The shared release executable was subsequently in use. An isolated output linked successfully without stopping that process or editing its source:

```powershell
cargo rustc --locked --offline --profile release-fast -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype --target-dir target -- -o C:/dev/voxel_v2_backup/target/ossuary-review/final.exe
```

`target/ossuary-final-build.log` records the successful final build and shader validation. `target/ossuary-build.log` retains the earlier blocked default-output attempt. Use the final executable above instead of assuming the mutable shared release binary matches this work.

`delivery-Engineering` uses the final executable above, with an identical frozen-copy hash and exit code 0. Both complete region readbacks passed: injured actor at 334 HP/loading damage 26/stock 5, then save v38 completion with exactly one drill. That count stayed at one through 27.51 s. This disk-I/O review is excluded from performance acceptance.

`r8-Combat` ran the earlier candidate through a complete ordinary-bolt destruction. `r8-Engineering` disabled both drives, secured one drill and read back an injured v38 actor. Its short wall-clock interval ended before the second save readback; `final-capture-Engineering` completed both full readbacks: injured actor (334 HP, loader 26, stock 5), then v38 completion with exactly one drill. The one-drill count remained stable through 27.51 s.

Local measurements use RTX 4090/Vulkan, 1280x720, 1 m voxels, view distance 4, static first-person route, clear weather, four-second warm-up. These are desktop development measurements, not the Iris Xe acceptance required by ADR 0017. Checkpoint runs are excluded from timing comparisons; whole-run combat averages include the period after victory.

| Run | Average FPS | Frame p95 / p99 / max, ms | GPU passes, ms | Gameplay CPU average, ms |
|---|---:|---:|---:|---:|
| r6 baseline, no Engine | 263.60 | 4.531 / 5.002 / 515.506 | 2.840 | 0.241 |
| r6 cutting | 275.31 | 4.530 / 4.792 / 329.131 | 2.979 | 0.139 |
| r8 final combat | 261.41 | 4.541 / 5.016 / 463.766 | 2.727 | 0.351 |

The r6 pair shares executable hash `8939C34C125EDFEC6B0D2B46B2D70825484EE8B3492455CE5FD04714E3EE7D84`. Its admitted-edit receipt records two requested/two applied cells in 0.032 ms. `persistence-Cutting` uses that same build and full region save/readback: v38 restored cut count 2 and cargo 2.

The final candidate differs from r6 in attack coordination and concise prompts; its completion review is r8. No pre-change original-Engine benchmark or median-of-three regression gate was recorded. Transient slow runs (r4: 48.86 FPS cutting, 67.04 baseline) and large stalls even in no-Engine controls prevent a causal performance claim. The admitted edit callback itself was fast; the source of the long frames is unresolved. Do not sum overlapping CPU/GPU timings or call this target-device acceptance.

Review PNGs include `r8-Combat/bench-000.png` (bracing and a readable committed cutting lane), `r4-Disabled/bench-000.png` (failed loader), and the before/after partitions in `r6-Cutting`. No uncut video was recorded.

## Reproduce

```powershell
# To review the preserved candidate:
$exe='target/ossuary-review/final.exe'
./tools/Review-Ossuary.ps1 -Executable $exe -Mode Combat -Seconds 32 -Label fresh-combat
./tools/Review-Ossuary.ps1 -Executable $exe -Mode Engineering -Checkpoint -Seconds 16 -Label fresh-capture
./tools/Review-Ossuary.ps1 -Executable $exe -Mode Cutting -Seconds 16 -Label fresh-cut
./tools/Review-Ossuary.ps1 -Executable $exe -Mode Baseline -Seconds 16 -Label fresh-baseline
./tools/Review-Ossuary.ps1 -Executable $exe -Mode Disabled -Seconds 12 -Label fresh-art
```

Manual mode uses normal controls in a disposable world. Evidence directories are never overwritten; normal save slots are not used. Checkpoint runs include disk I/O and synchronous readback, so exclude them from frame-time comparisons.

## Remaining acceptance

- Manually play direct combat and a prepared containment/sabotage approach. Examine chase/turn behavior, player/hull contact, service-side visibility, escape space and sound; revise if ordinary combat remains repetitive.
- Revisit a real generated site after unloading and player death; validate old-save upgrade and full-inventory completion under actual play.
- Install and power the recovered drill elsewhere, connect storage and demonstrate useful production. The reward uses existing factory hardware; this pass does not add a new tool-casting assembly or a new wand operator.
- Review 0.5 m and 0.25 m worlds, the actual Iris Xe target, and the long-frame source before claiming performance acceptance.
- No manual play session, uncut gameplay video, sound audition, new magical progression boss, ecological redesign or whole-roster quality claim is supplied here. No unit tests or hosted CI were run.

## Build and artifact status

- Release-fast compilation and isolated linking succeeded for final.exe; composed WGSL validation also passed. The shared default output was left alone when locked by another process.
- Normal saves were not used for reviews. The task's duplicate review executables were compressed in place after the disk filled; content hashes and evidence files were preserved. `r7-Combat` is an incomplete copy attempt and is not a valid candidate.
- No unrelated dirty source was reset or cleaned. No tests or CI were run; existing fixtures were adjusted only for the added tick argument and arena version field.
