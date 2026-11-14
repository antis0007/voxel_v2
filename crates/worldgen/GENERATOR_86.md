# Generator 86: inhabited room detail

Libraries and scriptoriums gain low reading seats on the outer side of their
desks. The shelving access strip stays clear. Refectory end bays gain timber
sideboards. Existing doorway, central aisle, gallery approach and reward-cache
clearance rules take priority over these furnishings.

The new furniture is gated at version 86; older saves retain their layouts.
Furniture dimensions remain in metres across voxel scales. Seats and sideboards
are editable decorative blocks, without a new sitting or storage interaction.

Bookcase cloth spines gain five heights and sparse binding bands. Bookcase and
chest detail now filters at its actual 20 texels/metre frequency. These material
changes apply to existing worlds too, preserving the established palette.

Validation: native build and all 22 composed shaders passed. Half-metre native
captures were inspected in `target/place-review/interiors86-library-half` and
`target/place-review/interiors86-refectory-half`. They show the main aisle and
dining bays, but do not establish seat placement across seeds, traversal or
interactive furniture. The review binary uses an app-only opt-level 1 override;
these captures are not performance evidence. No tests or hosted CI were run.
