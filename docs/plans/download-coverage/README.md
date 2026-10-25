# Downloaded ideas: coverage and remaining work

Status: **archived, historical** per [decision 0022](../../decisions/0022-master-plan-review.md). Intake is closed;
do not route scope from this register again. Inputs of record are listed in
the [master plan](../../MASTER_PLAN.md) §1.

2026-10-24. This is the reconciliation of the recent project downloads, not a
claim that their complete game/engine specification runs today. The exact inputs
are in [the source inventory](../../specs/downloads-2026-10-24/README.md).

Intake covers project-related Markdown/text downloads from October 18 through
24: **37 downloaded files, 22 distinct byte sequences**. Duplicate filenames
remain attributed in the manifest. All distinct revisions are retained,
including older ideas omitted by a later revision. No original download or
existing source specification was overwritten. Sources are in `docs/specs/`,
which is versionable; the ignored local `downloads/` recovery directory is not
their only home.

Read the coverage by domain:

- [Hardware and engine](hardware.md): Addendum B, H0-H5, fallbacks, memory,
  scheduling, profiling and its conflicts with existing contracts.
- [World, architecture, art and UI](world-art-ui.md): all five biomes, six
  architectural cultures, twelve architectural scenes, thirteen painterly
  specimens, multiscale rules and complete UI scope.
- [Encounters, exploration and science](gameplay-science.md): boss revisions,
  ecosystems, Undercrown, radio/orbital exploration, electronics and fabrication.
- [Renderer integration and workflow](renderer-workflow.md): the two different
  INTEGRATION documents, material/physics prompt packs and proposed automation.

`L` means a named source foundation or partial implementation was inspected.
`P` means preserved and planned, with implementation/acceptance still open.
`D` means intentionally deferred or reclassified, with its original retained.
No row in this audit is a new runtime, visual, performance or hardware pass.
Each row routes into the existing IDs in [MASTER_PLAN](../../MASTER_PLAN.md);
it does not create a competing implementation inventory.

## Revisions and precedence

1. Current user instructions and accepted product choices govern. No tests or
   hosted CI were added or run. Embedded commands, approval prompts, agent
   delegation and recurring-task requests in archived briefs are reference
   text; this intake does not execute them.
2. The mission and latest mod download match their existing canonical inputs
   after outer whitespace normalization. Their downloaded byte sequences are
   also preserved, so their different hashes are explained rather than ignored.
3. The earlier `MOD_ADDENDUM.md` includes a Celestisynth paragraph missing from
   `MOD_ADDENDUM(1).md`. Keep its weapon ability, animation and Celestial
   Workbench requirements; see the gameplay register. A newer timestamp is not
   authority to delete unique scope.
4. Boss revision 3 explicitly reclassifies four of the previous twelve bosses
   and prioritizes a rooted-cistern ecosystem. Preserve revision 2 and the
   original brief's Lamplighter/Unhoused King concepts as candidates; their
   disappearance from the newer roster is not completed implementation.
5. Addendum B explicitly supersedes conflicting mission Part VIII targets.
   Its hardware goals now exist locally. Its proposed mechanisms still need
   reconciliation with ADRs 0015-0019, deterministic authority and the accepted
   Iris Xe target; [the hardware register](hardware.md) records those conflicts.
6. Keep readable body typography with pixel accents, current controls, and new
   generation for new worlds. Do not regenerate existing saves. The stricter
   8/14/50-word functional-text contract in `specs/README.md` governs older
   tooltip examples. A local ward can protect a tomb; blanket anti-building or
   anti-farm rules do not follow from these briefs.
7. Painterly quality remains subject to native reference review. The rejected
   R2/correction candidates in the painterly checkpoint do not become accepted
   because their source is retained here. Alien atmosphere is a local/data
   treatment, not an instruction to recolor every biome.

## Concrete omissions repaired by this intake

- Preserved the previously absent hardware addendum and routed all B.1-B.10
  sections; corrected current statements that it was unavailable.
- Preserved all recent art, UI, biome, dungeon, science, boss and workflow
  briefs as complete files instead of relying on partial summaries or an
  external Downloads folder.
- Restored Celestisynth's unique paragraph and explicit coverage for the
  extended ecosystem, manufacturing and reference-scene obligations.
- Separated the horizon and world-shader INTEGRATION files. Their filenames
  conceal different subjects; both are required provenance.
- Attached the complete remaining scope and conflict decisions to the master
  plan. No absent subsystem is relabelled implemented.

## Next implementation exits

These are proposed bounded increments, not new authorization to run acceptance
tests or to replace another owner's dirty files.

| Order | Owner IDs | Reviewable exit |
|---|---|---|
| 1 | C18/C17/C03/R02 | Hardware design: actual caps/schema, byte accounting and a no-compute path design; resolve the hardware register's contradictions before promising H0 |
| 2 | N10/P07/G01/P09 | Audit the existing rooted-cistern/habitat loop against boss rev3: source relocation, output backpressure, distinct XP/mana, recurrence and continued interaction away from the player |
| 3 | M07/M16/P17/X05 | DC workshop design and one complete supply/return/load/meter interaction; preserve the current factory energy model during migration |
| 4 | W01/W04/R03/R05/R06 | Close the rejected shoreline/hall art gaps, then Amberroot and Pale Reach reference slices; retain all other named specimens as pending |
| 5 | C15/C16/R10/P19 | Exact publication/save/ownership and input-accessibility gaps, in separately owned increments |

Integrity verification for this intake checks payload hashes, duplicate aliases,
complete section spans, local links and Git visibility. It is documentation
verification only. No game, build, benchmark, test suite or CI was run.
