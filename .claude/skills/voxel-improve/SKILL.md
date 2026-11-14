---
name: voxel-improve
description: One focused engine improvement from source or runtime evidence, with a single writer, verification when requested, and explicit evidence gaps.
---

# voxel-improve

One pass, one issue, one evidence-backed result. Single writer: do not start another editing agent in this worktree.

1. State. Inspect status only for the task's paths; preserve unrelated dirty files. For cross-file questions use `node tools/axiom-dev/context-cli.mjs "question" [paths...]`; for known symbols use a bounded search. Read `tools/axiom-dev/CODING_TOOLS.md` only when tool routing is needed. Consult the relevant section of `docs/PLAN.md` only for roadmap tasks.
2. Pick one issue with user-visible impact from existing evidence: source behavior, a reported failure, measurement, or inspected capture. If runtime verification is requested, take one relevant baseline using the engine workbench or matching review tool (unique label, never overwrite). Otherwise state the evidence gap. One game run at a time.
3. Write the acceptance condition before editing (what capture, number or test must change, and what must not regress). Keep the baseline artifacts under `target/`.
4. Implement the smallest focused change. Load only the rule or skill for the subsystem touched (`.claude/rules/`, `render`, `voxel`, ...).
5. Inspect the focused diff. Do not add/run tests or hosted CI unless requested; do not launch the full workflow verifier by default. When verification is requested, choose the smallest relevant check. For measured visual/performance claims, compare the same pose/route and report frame-time statistics with warm-up excluded. Use one bounded review only when a contract change needs independent analysis. Record only checks actually performed; leave unmet checks unverified.
6. Report: what changed, evidence before/after, which checks ran against the final patch, what remains unverified. Add one lesson (trigger, violated invariant, smallest check) to the relevant `.claude/rules/` file only if a real defect was reproduced and fixed.

Never claim a visual or performance result without the captures or numbers; say UNVERIFIED instead.
