# Greedy and binary greedy meshing

Reviewed 2026-10-24; R01/R02/R13. [Lysenko's article](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/) compares naïve, culled and greedy quad generation. [Binary Greedy Meshing v2](https://github.com/cgerikj/binary-greedy-meshing) documents bit masks, material-aware merges, packed quads and vertex pulling. Its Ryzen result is author-reported and is not our hardware result.

## What we adopt

Use binary occupancy/face masks and greedy merging as one S3 candidate. Merge only when material, transparency, lighting and state attributes agree. Retain small reference inputs for comparing surface coverage and update latency.

## What we reject

Do not conclude that integer simulation requires integer rendering vertices or forbids baked AO. Do not transfer a 64-cell implementation's timings to 8³ bricks or a laptop iGPU.

## What we improve

Measure upload bytes, fragmentation, quad count, GPU time and edits-to-visible latency against raymarching. CPU mesh plus supported draw submission is a fallback candidate, requiring its own capability evidence.
