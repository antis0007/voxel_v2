# 0020 Recover scattered work without replacing newer authority

Date: 2026-10-24. Status: recovery and documentation policy for this task;
no gameplay migration or new dependency.

The user supplied the mission specification, mod addendum and game bible and
requested parallel recovery, cleanup and an implementation attempt. Preserve
the originals in `docs/specs/` and use `docs/MASTER_PLAN.md` as the reconciled
entry point. The later, stricter description rules govern functional text.
The current no-tests/no-CI instruction takes precedence over embedded recipes.

The current checkout contains extensive uncommitted work. A branch being
unmerged in Git is insufficient evidence that its behavior is missing: some
parallel work was copied or integrated under different commits. Recovery must
compare source and dependencies, preserve tracked/index/untracked changes with
hashes, and distinguish archived, already integrated, and conflicting work.
Artifacts are under `downloads/recovery/2026-10-24/`; the tracked recovery
inventory records provenance and restoration prerequisites.

Keep original worktrees, saves and source until every unique change has a
disposition. Do not use reset, blanket checkout, stash, clean or worktree removal
to make the status appear tidy. Do not import another implementation of a core
concept merely because a recovered directory contains it. Recovery archives
can contain tests as historical source; this task neither activates nor runs
them. Only compatible, reviewed production changes may enter the active tree.

The live source identifies save 32 and generator 46. Older v29/v30 and
generator-36 roadmap references cannot reserve or roll back a version. Future
migrations read and advance the live constants with explicit compatibility
evidence; no save/generator constants change in this recovery task.

Rejected: merging every unmerged branch (duplicates and save-ID conflicts),
deleting temporary directories after a superficial scan (untracked work can be
lost), and treating the 12–18-month specification as completed by a document
or compile result. Keep separate source, compile, runtime and hardware evidence.
