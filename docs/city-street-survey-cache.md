# Bounded street surveying

## Scope

Street A* now caches its clearance result and sampled elevation per coordinate within one request. Each nontrivial query with open endpoints allocates two bounded 6,045-entry vectors; no cache survives into another road, width, terrain snapshot or construction-planning poll. Missing elevations and blocked cells are cached too.

The original direction-state costs, deterministic tie breaks, path reconstruction, corner checks and 20,000-expansion limit remain unchanged. Endpoints outside the router's bounds are rejected before callback invocation or indexing. No geometry, generator gate, serialized state, material authority, active population or playable alias changed.

Callbacks must describe a stable survey during one synchronous search. The district and infill callers inspected for this change satisfy that condition. A future callback whose answer changes between calls must not be passed to this cache.

## Focused evidence

A standalone Rust test harness compiled the actual shared route.rs with the same Point, Profile and grid dimensions, without compiling the application or its dependencies. Seven tests passed in 3.83 seconds on 7 November 2026.

- 288 old/new comparisons across three profile costs, six seeds, four street widths and four terrain cases: 61 successful routes and 227 rejections. Paths, error strings and expanded-node counts matched exactly for these valid-endpoint cases.
- Missing terrain and blocked clearance are evaluated once per queried coordinate.
- Fresh queries observe changed exclusions, elevations and width requirements.
- Extreme and boundary endpoints are rejected before callbacks.
- Corner-cutting rejection, blocked endpoints, zero-length routes and the expansion-limit failure are retained.
- A nine-metre street around a river exclusion retained the same path and 18,772 expansions while reducing clearance callback calls from 296,838 to 3,410, direct route-height calls from 295,010 to 3,212, and total source-height samples from 24,483,184 to 271,170.

Those are operation counts in one deterministic fixture, not rendered FPS, overall settlement speedup or evidence of a larger simulated city. Immediately blocked endpoints and zero-length paths return before allocating the cache. Cache allocation can still outweigh savings for very short successful queries. No universal timing claim is made.

An independent read-only review of the cache found no actionable correctness defects. Its immediate-failure allocation concern was then addressed with early returns and one additional tested regression. Actual district doorway taper, building/timber reservations and live incremental-infill callbacks remain application-integration coverage to run with the next shared executable. Workspace tests, Clippy, benchmark baselines and full native city acceptance were not run for this increment.

## Reproduction and provenance

Task evidence: D:\codex-scratch\city-route-survey-20261107.

The folder contains the original and final routing sources, the test module, standalone harness and executable, focused-tests-final.log, source-manifest.json and street-survey.patch. Run the retained harness with:

    rustc --edition=2024 --test -O route-harness.rs -o route-harness.exe
    .\route-harness.exe --nocapture --test-threads=1

Original route SHA256: 3F9B3BD41D6D673D65F6106D595074DD05C3F23220CC33121A3CE86B5F0C6482.
Final route SHA256: 0C198E870E68C8495DD5D45A1E838E12FFBABE0BE04FCABC4C18B484F20D9265.
Final test SHA256: A401505906E65BF68997A4AE0B00B82E50B73A8DAF3904E292915D395D636DEE.

The shared checkout already had untracked recovered city sources, including route.rs. This increment preserves the baseline and narrow patch rather than claiming ownership of, staging or committing those recovered sources. The two implementation files were frozen for the separate checkpoint-diagnostics build. No save or promoted executable was overwritten.
Repository session-report completed. Its recovered-tree totals and historical benchmark rows cover unrelated work and are not results of this increment.
