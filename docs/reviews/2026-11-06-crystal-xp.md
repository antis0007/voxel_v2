# Crystal XP recycling fix

Crystal placement uses the existing bounded placement ledger. Mining captures
that record before removing the cell. Placed crystal returns its material and
consumes normal tool wear, but gives no mining XP. Generated crystal retains its
existing XP reward. Replacing tracked crystal clears its record.

The writer formats stay at checkpoint 67, block data 5, progression 4, network
protocol 14 and gameplay image 10. Terrain generation is unchanged. The ledger
already accepts arbitrary bounded cell coordinates, so stored crystal records
use its existing encoding. An older binary can read these files, but it does
not enforce the new XP rule. Crystal placed before this fix has no provenance
record and can still award XP once; subsequent placement records it.

## Regression

Run the reviewed isolated binary with `--crystal-xp-review`. The fixture sets survival
rules explicitly and calls production mining, recipe staging, crafting and
inventory synchronization. It checks natural XP, three craft/place/mine cycles,
unchanged material quantity, placement-ledger round trips and record cleanup.
The initial natural cell is explicit fixture setup. This is a native logic
regression, not a manual graphical playtest. The runner does not open a player
save or publish a checkpoint.

The shared mining module registers the same case as a unit test. The review CLI
stays in the isolated build to avoid changing the actively edited shared main
module. The full app test target currently
has unrelated compilation failures in renderer/UI tests and old signatures.

## Separate pending migration

The wider equipment-wear and multi-scale mining candidate remains isolated.
It proposes checkpoint 68, block data 6, progression 5, protocol 15 and gameplay
image 11, with legacy readers and an old-balance path. Older binaries cannot
read those new checkpoints. That migration needs independent save-format review
before integration; it is not part of this format-preserving crystal fix.

No actual player saves were upgraded during validation.
