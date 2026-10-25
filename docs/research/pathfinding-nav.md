# Navigation summaries and hierarchical paths

Reviewed 2026-10-24; N03/N04/N07. The [HPA* paper](https://webdocs.cs.ualberta.ca/~mmueller/ps/hpastar.pdf) redirected to an inaccessible author host. [Recast's project documentation](https://recastnav.com/) confirms a navigation-mesh toolset; its API/integration is not selected here. Dwarf Fortress connectivity and Minecraft goal-selector internals remain unverified reference leads.

## What we adopt

Propose dirty-brick connectivity/portals, a hierarchical graph and local refinement with integer costs and pinned tie breaks. S5 compares summary rebuild and route service under collapse bursts.

## What we reject

Do not equate static HPA* preprocessing with free updates in mutable terrain. A cached path is invalid after an intersecting geometry revision; corridor output alone proves neither clearance nor crew reachability.

## What we improve

Publish dirty backlog, rebuild cost, route failures and deterministic service order. Summaries carry source revisions; jobs retry or fail explicitly when snapshots expire. The N04 SDD chooses the abstraction after S5 numbers.
