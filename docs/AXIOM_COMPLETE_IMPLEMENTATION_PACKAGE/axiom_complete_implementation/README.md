# Axiom / Meridian — complete implementation handoff

Prepared from the visible conversation, its attached specifications and supplied reference art, and relevant retrieved earlier Axiom project context. This package is an implementation directive, not an implemented game update.

## Start

Give the coding agent `AXIOM_COMPLETE_IMPLEMENTATION_PROMPT.md` inside the existing game repository, and use `IMPLEMENTATION_AGENT_KICKOFF.txt` as the session instruction. Provide the package's `reference_art/` folder for visual work. The master is self-contained; the older documents listed in its provenance appendix are not required to reconstruct the main requirements.

## Contents

- `AXIOM_COMPLETE_IMPLEMENTATION_PROMPT.md`: one canonical master, 27 numbered chapters (0–26), explicit conflict resolutions, native implementation contracts and dependency-gated delivery.
- `IMPLEMENTATION_AGENT_KICKOFF.txt`: concise session-start instruction.
- `AXIOM_ACCEPTANCE_MATRIX.json`: 122 traceable acceptance cases across 14 categories, initially NOT_RUN with empty evidence. Repository runs now update individual cases with evidence; REN-01 remains failing/open. This is a tracking catalog, not executable test code.
- `reference_art/INDEX.md` and `REFERENCE_MANIFEST.json`: guide and provenance for 23 unique supplied reference images, preserving 29 upload aliases. Reference art is not licensed production content.

## How to use the scope

Read the master once as the target, then implement the earliest unmet gate in the audited repository. Consult subsystem chapters as needed. Do not attempt all chapters in a single unreviewable patch; do not reduce the target to disconnected placeholder demos. Preserve tested existing systems and the same home-to-expedition experience.

Future user changes should become explicit amendments or revisions to the master. Update dependent acceptance cases accordingly. The matrix is subordinate to the master and must not grow into a competing specification.

## Evidence boundary

No game source was inspected, changed, built, launched or benchmarked in preparing this package. File consistency and packaging were checked; that does not establish any engine capability. Native implementation, visual assessment, cross-machine tests, flight validation and performance measurement remain the coding agent's work in the actual repository.
