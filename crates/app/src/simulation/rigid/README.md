# Voxel rigid bodies

`rigid.rs` owns residency, swept motion, support, capture and sleeping.
`contact.rs` reduces each participant/normal pair to four face extremes and
solves accumulated normal and Coulomb friction impulses over eight iterations.
Mass and full world-space inverse inertia are cached for each solve. Restitution
and friction come from both materials; the bounce threshold is in metres/second.
Resting faces receive gravity support through the same solver as impacts.

`fracture.rs` compares available impact energy with the hardness-weighted cost
of cutting face bonds. It searches a bounded set of planes near the strike and
through the centre, then finds the connected components on either side. Pieces
inherit the parent's point velocity and angular velocity. Splitting changes
ownership without changing any world cell or granting loot. Capacity exhaustion
leaves the parent intact. Maul strikes, radial impulses and collisions share it.

Construction materials listed in `fragment_material` can detach when an edit
wakes them and their complete connected component has no static anchor. Natural
terrain is excluded. A rigid core still opts a connected construction into motion.
Factory cells and protected cargo are excluded. Loading a chunk alone does not
start a structural scan. Thermal processing continues after capture attempts.

Current limits remain 64 bodies, 128 cells per ordinary assembly and 4,096 cells
per felled tree. Discovery larger than the assembly cap stays static. Contact
work is capped at 16 patches, each with at most four constraints. Sweeps remain
bounded by translation plus tip speed, residency and the simulation wall budget.
This change adds no serialized fields or save migrations.

This is a voxel-lattice solver: the rendered collision geometry still changes
on cell boundaries, not as continuously transformed meshes. It does not yet have
whole-scene constraint islands, persistent contact warm starting, accumulated
bond fatigue, arbitrary large-building collapse, joints, or a rigid/fluid volume
transfer contract. The world setter remains the authority for environmental
admission; an atomic multi-cell rigid/environment transaction is still needed.
Those are substantive remaining gaps before claiming Teardown-level behavior.

Compilation is source validation. Stack stability, sliding, toppling, high-speed
impacts, fracture under repeated strikes, chain explosions and save/reload need
runtime evidence before physics quality or parity can be claimed.
