---
name: voxel-improve
description: One bounded autonomous improvement pass on the voxel game: pick one reproducible issue from evidence, fix it with a single writer, verify with the task controller, report the measured outcome.
---

# voxel-improve

One pass, one issue, one measured outcome. Single writer: do not start another editing agent in this worktree.

1. State. `git status --short`, `python -I scripts/workflow.py status`. Preserve unrelated dirty files. Read `docs/PLAN.md` for known debt.
2. Pick one issue with user-visible impact from real evidence: a failing test or scenario, a measurement, or a capture you inspected. If none exists, take one baseline: `just run` for gameplay, or the matching `tools/Review-*.ps1` for visuals (unique `-Label`, never overwrite). One game run at a time; nothing else heavy running.
3. Write the acceptance condition before editing (what capture, number or test must change, and what must not regress). Keep the baseline artifacts under `target/`.
4. Implement the smallest focused change. Load only the rule or skill for the subsystem touched (`.claude/rules/`, `render`, `voxel`, ...).
5. Verify. `python -I scripts/workflow.py verify` runs the checks the diff requires. Fix failures; at most two repair attempts, then stop and report. Capture the after state from the same pose/route and inspect the images; report p50/p95 frame times for performance claims, with warm-up excluded. Record manual checks with `python -I scripts/workflow.py attest <id> --note "..." --evidence <dir>`; for contract-level diffs request one read-only review of the actual diff, fix, and attest `review_contract` / `review_save`.
6. Report: what changed, evidence before/after, which checks ran against the final patch, what remains unverified. Add one lesson (trigger, violated invariant, smallest check) to the relevant `.claude/rules/` file only if a real defect was reproduced and fixed.

Never claim a visual or performance result without the captures or numbers; say UNVERIFIED instead.
