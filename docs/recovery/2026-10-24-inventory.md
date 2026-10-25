# Recovery inventory — 2026-10-24

This inventory records source recovery, not gameplay or performance acceptance.
No source checkout was reset, moved, deleted, cleaned, or overwritten. No tests,
game, or hosted CI were run for this inventory. Recovery files are under
`downloads/recovery/2026-10-24/` (normally ignored by Git).

## Baseline and recovery format

The shared repository baseline was `af9842ef7af5ea1d69577246936daccb5384734c`
on `growth-xmb`. Its dirty working tree belongs to the ongoing integration;
external commits were compared with that baseline and current source before
proposing integration. Non-ancestor commits can already be present through
equivalent edits: ancestry alone is not evidence of missing functionality.
Concurrent work advanced HEAD before the final capture to
`4837411aa8d3159cce64847ebe5b42101f603acd`; the final per-worktree metadata records
that actual bundle exclusion baseline. Original source branch HEADs stayed as
listed below. Recovery did not create or move those branches.

`external-worktrees/inventory.json` records all 13 linked external worktrees.
Each directory contains original HEAD/branch, status, absent commit list,
binary full-index worktree and index patches, and exact copies of modified or
untracked source files. Before/after status lists matched for all 13 captures.
Exact file copies preserve bytes even when Git normalizes line endings and
produces an empty patch. `.claude/verify/` task metadata is listed as skipped.

Seven incremental `commits.bundle` files preserve branch commits absent from
the baseline. **They depend on existing history in this repository and are not
standalone backups.** Each bundle passed `git bundle verify` against this
repository. Its actual prerequisites are recorded by the bundle itself;
the metadata `bundlePrerequisiteBase` is the exclusion baseline used to build it.
The external-worktree archive initially totaled 842,955 bytes.

The independent old `C:\dev\voxel_v2` checkout has its own complete-history
`legacy-voxel-v2/history.bundle`, index/worktree patches, status, and SHA-256
manifest. Its HEAD is `5fb9f4b3d2512e4fbc487c4eebfa22172a1a54da` (`main`, `init`).
Its many dirty renderer files and deleted planning docs were preserved without
merging an obsolete single-crate layout over the current workspace.

## Linked worktrees and disposition

Paths below are literal original paths. `TempRoot` means
`C:\Users\antis0007\AppData\Local\Temp\claude\c--dev-voxel-v2-backup`.

| Original path | HEAD / branch | Absent commits / dirty entries | Disposition |
| --- | --- | --- | --- |
| `C:\AI\pyrenet-work\wt-chests` | `6dbf8fa9`, `chests` | 3 / 1 | Four chest tiers, inventory and crafting table; source preserved. Save/material-ID and newer inventory overlap require deliberate integration. Dirty entry is verification metadata. |
| `C:\AI\pyrenet-work\wt-flame` | `bee89bf9`, detached | 2 / 0 | Fire ramp and flame heat field already present in current shader. Archive, do not replace newer shader. |
| `C:\AI\pyrenet-work\wt-mine` | `6e162de4`, `spells-followup` | 0 / 0 | Commit ancestry already included. |
| `C:\dev\voxel_a5` | `11b2a26e`, `a5-integrate` | 0 / 3 | WIP consists of GPU review tests and test wiring. Preserve only; no tests activated under current instructions. |
| `C:\dev\voxel_integrate` | `ec84be2c`, `integrate/parallel-2026-10-24` | 8 / 0 | Additional standalone GPU CA implementation in `crates/gpu` overlaps existing `sim_gpu` purpose. Preserve, do not introduce another active authority. |
| `C:\dev\voxel_latency` | `ffa9c94a`, `latency` | 3 / 0 | Player-edit urgent meshing lane absent from current source; approximately 1,045 additions across 10 files. Strong future candidate, with substantial dirty streaming/main overlap. Historical timing claims need new measurement. |
| `C:\dev\voxel_orbit` | `40db53b4`, `orbit-clock` | 1 / 0 | Shared orbital clock already incorporated; newer orbital source adds hundreds of lines. Do not restore older files. |
| `C:\dev\voxel_v2_wt_atmo` | `1a1e26ea`, `atmosphere-config` | 5 / 0 | Main atmosphere implementation present. Production finite-value validation and panic-free preset fallback recovered from `5259f997`; optional review override remains archived. |
| `C:\dev\vx_igpu` | `af9842ef`, detached | 0 / 2 | Shader and `world_shader.orig` preserved byte-for-byte. Git-normalized shader patch is empty; exact copies matter. |
| `TempRoot\2c6f90e3-bdcb-46ff-bd0f-5fff4dbe664c\scratchpad\t4\ab` | `6a0810cc`, detached | 0 / 0 | Baseline history already included. |
| `TempRoot\2c6f90e3-bdcb-46ff-bd0f-5fff4dbe664c\scratchpad\t4\ab-b` | `ded722bd`, detached | 1 / 0 | Multi-draw indirect implementation already present in current render source. |
| `TempRoot\dad60308-ffcc-4e37-bd9d-1eb9f13e8a92\scratchpad\bis` | `d5eb34a5`, detached | 0 / 1 | WIP adds diagnostic `eprintln!` probes in headless checksum; preserve, do not activate. |
| `TempRoot\f7d81971-21d3-4022-a6c7-67b329ea2e10\scratchpad\head-wt` | `2541dde4`, detached | 0 / 1 | WIP moves `FrameCounters` fields out of an accidental comment; archived historical repair. |

The recovered atmosphere code rejects non-finite planet radius and non-finite
or negative ozone center. A manually constructed empty preset list safely
falls back to built-in earthlike coefficients. Valid shipped configuration
keeps its existing behavior. No review tests were imported. Pre-recovery file:
`main-before/files/crates/app/src/render/atmosphere_config.rs`, SHA-256
`28C8AF0DFE43DA34CED91BAA316C92846C15432C116A3A565AE4974EF9E6CD59`.
Build status is reported by the main integration task separately.

## Unlinked source and historical documents

`source-archives/provenance.json` maps each original source path to its SHA-256
content blob. Repeated byte-identical files share one blob. Nothing in this
archive becomes authoritative current documentation merely by being recovered.
The collection includes source snapshots outside registered Git worktrees and
standalone draft patches, with these original roots:

- `TempRoot`: 50 session directories, including source copies such as
  `d0a43c76-dcf5-4759-bdd5-61c01f9e0777/scratchpad/v2_mine/v2`, standalone
  `c495acab-37bf-421b-a5fe-5924782faeae/scratchpad/{task12,owned_code}.patch`,
  and historical lore additions under `b5d18810-c7be-4597-8a5e-7dc6a74f46c4`.
- `C:\dev\voxel_v2_scratch`: `docs-archive-2026-10-22` has 104 historical
  documents totaling 1,013,529 bytes; `root-junk-2026-10-22` contains old horizon
  source/patches and launchers. Historical design documents stay in the archive.
- `C:\dev\voxel_v2`: older standalone source and documentation.
- `C:\Users\antis0007\Downloads\voxel_reupload`: original uploaded source,
  backups and cloud repair documents.
- `C:\Users\antis0007\Downloads\voxel_renderer_redesign_patch`: old renderer
  replacement source and change notes.

Final source archive: **6,867 original source paths, 1,673 unique blobs,
37,140,791 blob bytes; 40,179,801 bytes including provenance and manifest.**
Path counts are 6,306 Claude temporary files, 380 historical scratch files,
63 legacy source files and 118 downloaded source files. The historical scratch
count includes nested documents/source, not just the 104 top-level documents.
Two small historical source ZIPs (`horizon_terrain_upgrade.zip`, 55,285 bytes,
and `terrain_shader_upgrade.zip`, 119,469 bytes) are also preserved as opaque
archives; their contents were not executed or installed.

Selection includes Rust, WGSL, RON, TOML, Markdown, text, patches, Python,
PowerShell and `.orig` files up to 16 MiB. It excludes Git objects, build targets,
node modules and agent configuration. Executables, saves, images and full session
logs were not copied. This is a source recovery archive, not a complete machine
backup. `capture-source-archives.ps1` records the reproducible selection.

`ADDENDUM_B_HARDWARE.md` was not found by filename in the Claude voxel temporary
tree, historical docs archive, old voxel source or downloaded voxel source.
Do not invent its contents or treat this bounded search as a full-disk proof.

Later the same day, the Downloads intake found
`C:\Users\antis0007\Downloads\ADDENDUM_B_HARDWARE.md`. Its exact text is now
preserved under [specs](../specs/ADDENDUM_B_HARDWARE.md), with full download
provenance and other recent briefs in
[the source inventory](../specs/downloads-2026-10-24/README.md). This closes the
earlier input gap without changing the bounded search recorded above.

## Cleanup boundaries

The following neighboring folders have build-output layout (`debug`, `release`,
`release-fast`, `CACHEDIR.TAG`, `.rustc_info.json`) rather than independent source:
`voxel_latency_check`, `voxel_latency_target`, `voxel_tgt_bossaccess`,
`voxel_v2_mood_target`, `voxel_v2_wt_atmo_target`, `voxel_v2_wt_before_target`,
`voxel_v2_wt_seam_target`. `vx_igpu_bin` holds executable experiment artifacts.
They were not copied or deleted: performance capture binaries may still be
needed to reproduce comparisons.

The current Git object store reports approximately 5.37 GiB of packs and
1.91 GiB in 39 temporary garbage objects. No Git garbage collection, pruning or
object deletion was attempted. Concurrent Git activity and dangling recovery
history make blanket deletion inappropriate. Claude project logs include about
1.01 GB for the backup project; these are context logs, not source worktrees,
and were left intact.

Cleanup performed here is consolidation and deduplication in a traceable
archive, with current-source versus historical-candidate dispositions. Original
worktrees remain reachable. No destructive disk-space cleanup is claimed.

## Verification and restoration

Every copied source blob was hashed after copying and matched its source.
`SHA256SUMS.txt` files cover archive payloads and provenance. Bundle verification
is archive integrity/prerequisite verification, not a project test.

Read bundle prerequisites without modifying any checkout:

```powershell
git -C C:\dev\voxel_v2_backup bundle verify C:\dev\voxel_v2_backup\downloads\recovery\2026-10-24\external-worktrees\voxel_latency\commits.bundle
```

Restore a source file into a review directory, preserving originals:

```powershell
$archive = 'C:\dev\voxel_v2_backup\downloads\recovery\2026-10-24\source-archives'
$records = Get-Content -LiteralPath "$archive\provenance.json" -Raw | ConvertFrom-Json
$entry = $records | Where-Object source -EQ 'FULL ORIGINAL PATH FROM PROVENANCE'
# Choose a fresh review destination, then copy the exact blob.
Copy-Item -LiteralPath (Join-Path $archive $entry.blob) -Destination 'REVIEW DESTINATION'
Get-FileHash -LiteralPath 'REVIEW DESTINATION' -Algorithm SHA256
```

For a linked-worktree candidate, start from its recorded HEAD in an isolated
review checkout with the current repository history available. Restore the
recorded index patch with `git apply --index`, then its worktree patch with
`git apply`; restore exact copied files and untracked files afterward when byte
identity matters. Preserve recorded deletions. Do not apply recovery patches
wholesale onto the dirty active checkout. The old standalone bundle has complete
history and can independently seed a separate review clone.
