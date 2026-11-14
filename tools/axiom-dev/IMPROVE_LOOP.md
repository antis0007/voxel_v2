# Background improvement loop

Small rule-backed chores, found without a model and handed to the cheapest model that can do them. The loop is four commands; none of them calls a model, builds, or edits source.

```
node tools/axiom-dev/improve-cli.mjs scan [paths...] [--kind unwrap|todo|wall-clock] [--limit 20]
node tools/axiom-dev/improve-cli.mjs brief <id> [--max-chars 3000]
node tools/axiom-dev/improve-cli.mjs check <id> <diff-file>
node tools/axiom-dev/improve-cli.mjs done <id> applied|no-change|rejected [note]
```

- `scan` lists open chores as JSON, cheapest tier first. Test code, examples, build scripts, comments and the reserved dead-code files are skipped. Inline tests are found by the first `#[cfg(test)]` line in a file, so code after a mid-file test module is skipped too.
- `brief` prints a plain-text prompt of about 1,500 to 3,000 characters: the rule, the task, a numbered excerpt and a fixed reply contract (a unified diff, `NO CHANGE: reason`, or `NEED: more`). It is the same text for every model.
- `check` guards a returned diff: one file, the briefed file, at most 40 changed lines, no added `unwrap`/`todo!`, and `git apply --check` passes. It does not apply the diff.
- `done` settles a chore in `target/axiom-dev/improve-ledger.jsonl`. Settled ids never scan again, including `no-change` answers, so a judged-infallible `unwrap` is asked about once. The ledger is per checkout.

## Tiers

| Tier | Chores | Claude Code agent |
| --- | --- | --- |
| haiku-medium | stale `TODO`/`FIXME` notes | `improver` |
| sonnet-medium | `unwrap`/`expect` outside tests and startup | `improver-sonnet` |
| opus-low | wall-clock reads in simulation and generation | `improver-opus` |

A rule is one entry in `rules` in `improve-cli.mjs`: a ripgrep pattern, optional paths, the tier, the project rule it enforces and the instruction. Add a rule there; nothing else needs to change.

## Running it

In Claude Code, one chore at a time: `scan --limit 1`, `brief <id>`, pass the brief as the whole prompt to the tier's agent, then `done <id> ...` with its answer. The agents edit one file, run one `cargo check` for the owning crate and do not commit. For an unattended run, wrap that sequence in `/loop` with a long interval; keep one agent active, since every agent shares this working tree and the cargo target.

In ChatGPT or any other chat model: paste the brief, save the reply to a file, run `check <id> <file>`, apply the diff with `git apply` if it passes, `cargo check` the crate, then `done`.

## Limits

The scan is lexical. It cannot tell a provably infallible `unwrap` from a real one; the model decides and the ledger remembers. `check` proves the diff is small and applies, not that it is correct: compile it and read it. Nothing here runs tests or measures behaviour.
