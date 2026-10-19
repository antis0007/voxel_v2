# Organic world revision

Superseded visual direction: the user confirmed crisp cubic terrain on October
18 after resolving conflicting instructions across two active chats. The active
solid mesher uses cubic greedy faces; the organic mesh experiment is not wired
in. Preserve the horizon near exclusion and movement fixes independently of
the chosen terrain style. GPU review launches are paused for the other chat's
single-instance profiling.

Final cubic candidate: `target/release/cubic-caves-fixed.exe`, release build
completed successfully with shader validation. This candidate was not launched
because GPU profiling remains owned by the other active chat.

The visual direction is organic terrain. New worlds use generator 9: more
connected land, closer ridges and valleys, smaller trees, a smaller spawn
clearing, and landmarks at 192 m intervals instead of 224 m. Existing saved
generator versions retain their layout. Create a new world to see the layout
changes.

Natural mesh fan centres now follow their relaxed boundary instead of the
original cubic face. Corner relaxation is stronger while existing structural
and topology pins remain. This reduces diamond-shaped bumps; collision remains
voxel based.

Ground shading uses filtered world-space grain, vegetation clumps, darker
grass, gentler grassy slope normals, and less exposed dirt. Lowland snow is
reduced for generator 9. Distant ground shares the near terrain material
function and diffuse sunlight normalization. Sparse woodland proxies are
restricted to the far field; they are not editable tree instances.

The backdrop is now excluded from the inner resident region. Lowering its
surface alone was insufficient: it filled cave mouths and excavations wherever
the detailed mesh had an intentional opening. The prioritized inner 128-block
region excludes it (bounded by the draw radius). Outside that region, depth
handles overlap while outer chunks stream; caves beyond that range can still
be approximated by the distant surface.

Swimming now reaches the double-tap flight handler. Holding jump maintains
upward velocity through the water surface. These changes still require a
manual swimming/flight interaction check; renderer sweeps do not exercise
player controls.

## Review evidence

Release compilation and composed shader validation passed. Captures under
`target/organic-review/entrance` and `chamber` show the mine descent and chamber
without the heightfield filling them. These used the earlier wider exclusion.
The moving sweep exposed streaming gaps with that wide exclusion; it was
reduced to 128 blocks and rebuilt. `final-sweep` records the resulting landscape,
but was stopped at 17 seconds to avoid interfering with the other chat's GPU
profile. It also predates the final return to cubic meshing. No automated tests
or hosted CI were added or run. Cave ambient light and emissive mushrooms remain
too bright; these captures do not establish finished material quality.
