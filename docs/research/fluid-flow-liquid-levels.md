# Fluid flow and liquid levels (Gallant, Terraria, Minecraft)
Source: Jon Gallant, "Fluid Flow for the Rest of Us": unverified, not found by search. Terraria behaviour: terraria.wiki.gg/wiki/Liquid (behaviour only, update code unverified). Minecraft level-8 scheme: unverified, no source found.
Adopt: integer fill level per cell with conservative transfers (a cell hands out integer shares in a fixed order, so mass is exact); fall first, then spread sideways; a minimum-depth cutoff so thin films vanish.
Reject: float pressure-to-neighbour flow models; single-sweep updates in one direction, which the vendor-listed "teleporting water" bug suggests (unverified).
Why: Integer levels with exact conservative transfers keep the cellular state bit-deterministic across variable voxel scales and bound per-brick cost on the iGPU.
