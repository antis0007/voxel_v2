# Voluntary work circles

## Scope

This is an opt-in construction cooperation slice. Existing physical workers decide whether to propose, join, renew, leave or split a work circle. They select from funded construction work already admitted by the settlement or player. Circles cannot create orders, stock, cargo, building claims or residents.

Enable with `VOXEL_CITY_GROUPS=1` or `--city-voluntary-groups`. Once enabled, the world remembers it in its farming sidecar. Without opt-in, old saves retain their existing job behavior and omit the unused group field. The binary save version is unchanged.

## Individual choices

Each worker has stable seeded preferences for craft, belonging and autonomy. Observed nearby productive coworkers and completed physical social visits create sparse, symmetric relationships. Coworker contact requires distance under 4 m and line of sight.

A proposal does not assign its recipient. The recipient must independently see funded work in that area and accept on their own review. Refusals temporarily suppress the same offer direction, allowing proposals to other willing contacts. A member may leave a stalled circle for another opportunity. A resulting new circle keeps its parent identity.

Names are persistent mechanical identifiers such as `Work circle 3`. A changed shared goal does not give automatic consent: every member must renew it on their own review before it biases their task choice. Unaccepted, dead goals eventually allow members to leave rather than trapping them.

## Resource and scheduling boundaries

A shared goal only breaks ties among already-valid tasks of the same existing priority. Meals, food scarcity, rest, safety, carried cargo and job eligibility keep their prior precedence. Existing exclusive construction claims and physical chest/cargo transfers remain authoritative. Paid-work attribution runs only after successful voxel placement and payment.

The existing worker scheduler staggers reviews. Each worker reviews at most once per 480 fixed ticks (4 seconds at 120 Hz), with at most 16 material/geometry candidate checks. A separate goal-membership scan is bounded by the 256-order queue and reported separately. Contacts inspect the bounded 64-worker roster and retain at most 128 relationships. There are at most 16 circles, 8 members per circle, 64 pending offers and 32 recent events. Weak stale relationships can be replaced deterministically.

No wall clock or nondeterministic random source enters the decisions. Review deadlines, candidate cursors, relationships, refusals, accepted goals, identities and events persist in the farming sidecar.

## Verification

Run the native executable with `--city-group-check=<fresh-output-directory>` and set `VOXEL_SAVE_DIR` to `<fresh-output-directory>/disposable-saves`. It runs policy regressions, physical cargo and repeated-commit checks, an eight-worker autonomous construction fixture, and a full-world checkpoint round trip. The fixture writes `autonomous-groups.json` and `full-reload.json`.

The report distinguishes actual productive workers, physical paid blocks, accepted proposals, shared-goal selections, individual contributions, candidate checks and goal-membership checks. Timing covers the fixture's gameplay calls only, not rendering, the full cellular finish stage or a city-scale frame benchmark.

A separate exact-source policy harness can exercise the 64-identity limit, but that is not evidence for 64 physically active group-enabled residents.

## Not implemented in this slice

New construction siting by individuals, farming/fishing/logging goals, resource ownership by groups, gangs, violence, espionage, coups, politics, ideological beliefs, distant population simulation and a group-management UI remain future work. Current social preferences are simple numeric motives, not a full belief system. Full settlement performance with groups enabled still needs a dedicated city-scale acceptance run.
