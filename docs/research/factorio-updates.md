# Factorio optimization and simulation boundaries

Reviewed 2026-10-24; M05/M09/M11/C15. Developer sources: [FFF176 belts](https://www.factorio.com/blog/post/fff-176), [FFF416 fluids](https://www.factorio.com/blog/post/fff-416). The former stores item gaps along transport lines; the latter replaces local pipe dynamics with aggregate pipeline contents. The brief's FFF204/250/317/367 references do not establish the claimed fluid/update-order algorithms.

## What we adopt

Prototype compressed belt lanes and lazy stable-state work as M05 alternatives. Explicitly distinguish contained transport networks from cell fluids, and simulation decisions from visual updates.

## What we reject

Do not adopt aggregate network fluid behavior as evidence of open-world hydrostatics. Do not infer current-version behavior from a historical design blog or import author timings into our budget.

## What we improve

A single transport owner exposes conservative injection/extraction, visible failure and revisioned save/replay state. Measure congestion, splits, merges and destruction before claiming scale; signal programs share V01 rather than a second evaluator.
