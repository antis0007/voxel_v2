# Source specifications

The three initial files are byte-for-byte copies of the attachments supplied on
2026-10-24; the hardware input was later found in Downloads at the user's direction
and preserved unchanged. They remain reference inputs, including their original
planning instructions. The latest request resupplies the identical game bible:
research and the master plan now precede spikes and new implementation, with
human checkpoints after session 2 and the first passed core SDDs. This planning
pass adds no tests and runs no tests, benchmarks or hosted CI.

| File | Supplied origin | SHA-256 |
|---|---|---|
| `ENGINE_CORE_PROMPT_v2.md` | `0e71b975-fe32-4286-8a85-d8f83de91bff` | `76b7a2135efde84a2e0dd280ade1c3c5754fa1894d25f6dd32359e51763ba68f` |
| `MOD_ADDENDUM.md` | `6c1e4a8b-f35e-42a8-bf29-1f9f50cb0e93` | `416174be001e0d7bb2dccbd7502cd077888c8f0f8acc591bd5a67b3270c56a4b` |
| `GAME_BIBLE.md` | `c6e52811-c556-4833-8c4b-dace356baae1` | `43f869c8271f54d541203d38c12f1b1fcf8ff028d5e28ed8b2ca7134ecfd0534` |
| `ADDENDUM_B_HARDWARE.md` | User-directed Downloads lookup | `129902bc8f2dbc6866d6f4c54b604f8ff05a36a4bc3cf5cffe0a3c587a4c2c30` |

The resupplied attachment `1dd83eff-49f2-4a8a-bb03-2f632b0a9cd9/Pasted text.txt`
has the same SHA-256 as `GAME_BIBLE.md`; the preserved original needs no rewrite.
Process reconciliation is in [ADR0021](../decisions/0021-bible-planning-gates.md).

Read [the master plan](../MASTER_PLAN.md) for reconciled sequencing and
[the audit](../recovery/2026-10-24-spec-audit.md) for gaps. The later bible
advances the VM and Primer; the mod addendum expands the feature matrix.
Neither a roadmap nor recovered source establishes runtime acceptance.
The original attachment intake lacked `ADDENDUM_B_HARDWARE.md`. The later
Downloads intake found it and preserved an exact copy at
[ADDENDUM_B_HARDWARE.md](ADDENDUM_B_HARDWARE.md), SHA-256
`129902bc8f2dbc6866d6f4c54b604f8ff05a36a4bc3cf5cffe0a3c587a4c2c30`.
Its H0-H5 targets and B.1-B.10 scope are routed in
[the hardware register](../plans/download-coverage/hardware.md); measurements
and integrated fallback support remain unproved.

The [Downloads inventory](downloads-2026-10-24/README.md) preserves 37 project
downloads as 22 distinct texts with full hashes, all aliases and complete source
section spans. The [coverage register](../plans/download-coverage/README.md)
retains art/UI/biome/architecture, boss/ecosystem, science/electronics,
renderer-integration and workflow scope, including Celestisynth's omitted
paragraph. Archived commands are not authorization to run tests, install tools
or schedule work.

Worldgen input of record: [W-GEO/HYDRO/BIO](W_GEO_HYDRO_BIO.md), supplied by the
user on 2026-10-24 after the input freeze and recorded in
[decision 0023](../decisions/0023-worldgen-spec-input.md). Preserved as pasted.

Additional architecture input: [Project Axiom revision 4](project-axiom-revision-4.txt),
preserved from attachment `22b518d2-4fea-4420-b5de-e3db7573eef0` with SHA-256
`C705D4443A946E1593F1CBCAD0219EA8734BDD087B09FBC0F1C1320B96E8ACF1`.
Its [review](../design/PROJECT_AXIOM_REV4_REVIEW.md) and
[implementation track](../parked/2026-10-24-axiom-implementation.md) (parked for M1–M3 per decision 0022) refine engine
contracts and route the complete scope into the master inventory. This does not
replace the bible's lore or the functional-text rules below.

## Description rules (latest user revision)

The second IV.11a block in the request supersedes the first:

- Default: no description. Standard items show a name and relevant automatic
  stats only, including healing or food values.
- A hidden one-behavior twist earns one clause of at most 8 words.
- A mechanic earns one sentence of at most 14 words, with a separate cost line.
- Functional descriptions never grow into paragraphs or exceed two lines.
  Hold-to-expand codex detail is optional and at most 50 words.
- Present tense, active voice, no decorative adjectives. Values come from
  gameplay/material data, never duplicated numeric prose.
- Burning, Wet, Charged and Sealed refer to shared definitions.
- Delete every word that does not change a player decision. Empty is valid.

Adoption is tracked in [the tooltip design](../systems/tooltips.md); do not
infer that every existing spell, machine or codex surface has been migrated.
