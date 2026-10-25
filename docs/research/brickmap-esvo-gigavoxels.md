# Sparse voxel storage

Reviewed 2026-10-24; C03/C14/R10. [Laine/Karras ESVO](https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees) presents compact storage, contour/normal encoding and ray casting. GigaVoxels (Crassin et al., DOI 10.1145/1507149.1507152) and the brief's van Schaik 2016 Brickmap attribution were not independently fetched in this pass; do not treat them as mutable-world benchmarks.

## What we adopt

Retain the accepted 8³ production-brick unit while S1 measures alternatives in isolation. A sparse index, pooled pages and per-brick dirty/optional-plane metadata provide explicit allocation and wake boundaries.

## What we reject

Do not claim all octrees are unsuitable for editing, or use rendering compression as the authoritative simulation format without measured update costs. Do not infer on-demand plane memory from base-cell storage alone.

## What we improve

C03 computes pool/index/header/halo/free-list costs; C14 adds every allocated plane and history/mirror byte. Compare mutation, residency and eviction workloads before a new data structure is accepted.
