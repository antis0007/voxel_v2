# Generator 58: benchmark cottage shell

Only the valley settlement's role-2 workshop changes. `settlement/hall.rs`
owns its complete solid/air volume using a footprint, gable and opening plan.
There is one enclosed room, a continuous stepped roof, a framed open entrance,
side/rear windows with sills, floor, foundation and perimeter work/storage bays.
Earlier generator branches remain available; no save migration is performed.

The reference seed is 424242. Run `tools/Review-Architecture.ps1` against a
frozen executable for the camera suite. `--benchmark-debug=clay` supplies a
texture-free massing view. The review is not a player traversal test.

Native results, audit, measurements and outstanding acceptance requirements:
`docs/design/ARCHITECTURE_RECOVERY_20261026.md`. In particular, coarse-scale
roof proportions, material quality, site integration and performance regression
acceptance are not established merely by passing sampler tests.
