# Generator 60: finite flooded observatory

The existing Clifftop observatory gains a contained pool around its lens mount,
a breakable stone drain wall, an outlet below the pool floor and a lower basin.
The dome, stair, lens identity and archive reward remain the existing site.
Generators 1 through 59 retain their previous sampling paths.

Geometry lives in `undercrown/v2/observatory.rs`. Dimensions are metres relative
to the existing layout anchor. The pool is 7 by 6 by 1 m before its pedestal is
subtracted on the voxel lattice. The receiving basin has 84 cubic metres of
empty capacity below the pool floor. Its walls, floor and ceiling are authored
stone. The outlet's one-metre stone plug can be excavated normally. There is no
infinite-source rule, refill operation or drain-completion flag.

The pool's one-metre depth lets an ordinary pump stand in air directly above
its bottom water layer at all supported voxel scales. Powered extraction uses
the existing admitted finite buffer exchange, pipes and destination storage.
The excavation route uses ordinary conservative cellular flow into the same
lower basin. Either route changes persistent world water; neither toggles a
mechanism that deletes it.

Application interaction checks water at four contacts beside the lens mount.
Restoration waits until all four are at most one eighth full, allowing the
ordinary solver's shallow residual film. Restored lens state retains the normal
Undercrown identity and archive-opening behavior. Existing saves keep their
generator version and receive no terrain migration. Save format stays unchanged.

`VOXEL_REVIEW_DUNGEON=observatory` places the review camera in the dry west aisle,
with identical local eye/target points for generators 59 and 60. It only moves
the camera. It does not excavate, pump, restore the lens or prove persistence.

Build and render receipts are recorded in the Axiom execution ledger. The full
two-route traversal, conservation accounting, departure/reload and reward claim
still require runtime evidence.
