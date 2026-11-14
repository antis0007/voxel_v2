# 02: Reconcile the generator-gate registry and add a Manyfold tracking row

Written for: a coding agent. Read `00_INDEX_AND_SHARED_RULES.md` first. Docs-only.

## Goal

Make the written record match the tree after prompt 01: gates 80 to 90, the
Manyfold work, and its stage status are findable in one place, so no future
session has to dig through transcripts.

## Facts to start from

- `docs/worldgen/GENERATOR_GATES.md` is the registry. Its claim procedure: take
  the lowest free number, add a row, grep every branch and the shared tree
  before committing (commands are in the file header).
- Per-gate notes exist: `docs/worldgen/GENERATOR_{80,81,84,87,90}.md`.
  Code constants: `manyfold/weave.rs` VERSION 81, `manyfold/architecture.rs`
  VERSION 84, `manyfold/undercroft.rs` VERSION 87, the hell-gate approach 89.
- Memory `generator-gate-collisions` says gates 54 to 56 were claimed by several
  branches and the dirty tree used 54 to 65. Do not assume the file is current.
- `docs/TRACKING.md` is the roadmap tracking file (memory
  `elevator-tracking-and-orbital-captures`). `docs/MASTER_PLAN.md` governs
  priority; decision 0039 says briefs are backlog and `dam_break` (M1) keeps
  priority. Rows enter the plan under existing tracks only.

## Steps

1. Build the true gate table. For each number from 50 to the highest used:
   `git grep -n -E "version >= N[^0-9]|= N;"` across `$(git for-each-ref
   --format='%(refname:short)' refs/heads)` and the worktrees listed by
   `git worktree list`. Record branch, feature, status (live, dormant, bumped),
   and commit. Resolve collisions by renumbering the **dormant** branch's
   claim in the table and flag each affected branch owner in the report; never
   renumber anything in HEAD or live.
2. Fill gaps for gates that exist in code but not in the table. Do not invent
   history: write "unknown, found in <path>" when provenance is missing.
3. Add a `Manyfold` section to `docs/TRACKING.md` with one row per brief stage
   (Stage A tube traversal: landed or not per prompt 01; Stage 1 Foldfields and
   Stillcourt; 2 illusions; 3 stair fold; 4 Bellworks conditional arch;
   5 relic; 6 Long Guest; 7 districts), each with status, gate, prompt file in
   this folder, and blocker. Use `docs/design/MANYFOLD_TUBE_STAGE_A.md` and the
   brief's section 16 for stage definitions. Keep rows to one line.
4. Fix `CLAUDE.md` constants line only if the owner agrees: the file says save
   44 and generator 79 (checked 2026-10-29); read the real values from source
   and report the difference instead of silently editing orientation docs.
5. Add a link from `docs/MASTER_PLAN.md` to the new TRACKING section.

## Acceptance

- Every `version >= N` in `crates/` appears in the registry with an owner and
  status; `rg "version >= (\d+)" -o -r '$1' crates | sort -u` is a subset of the table.
- No new gate number is reserved by this prompt.
- `just test` still passes (some tests read docs, e.g. repo layout, TRACKING).

## Do not

- Do not change code or constants. Do not touch `docs/NEW/` (owner staging).
