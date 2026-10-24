# Brickmap, ESVO and GigaVoxels: sparse GPU voxel storage
Source: Laine and Karras, "Efficient Sparse Voxel Octrees", I3D 2010, https://research.nvidia.com/publication/efficient-sparse-voxel-octrees (confirmed; assumes static geometry); Crassin et al., "GigaVoxels", SI3D 2009, DOI 10.1145/1507149.1507152 (confirmed); van Schaik 2016 Brickmap: no primary source found, unverified (only a hobbyist devlog saying bricks are fast to modify).
Adopt: 8^3 bricks as the unit of storage in a GPU-resident brick pool with a shallow sparse index; out-of-core streaming for distant data; per-brick empty/solid bitmasks to skip work (community-reported, unverified).
Reject: deep pointer octrees with per-node allocation; ESVO contour and normal compression (static-geometry assumption); GigaVoxels ray-guided streaming as our mutation path.
Why: Octrees restructure on every edit and are poor for mutable cell state, while flat 8^3 bricks mutate in place and fit the low-end iGPU memory budget.
