# Player guidance

See [Sandbox play path](SANDBOX_PLAY_PATH.md) for the first-session design
backlog and friend playtest observations to collect.

Inventory has a visible Projects entry on the existing Goals page. Branches are
Expeditions, Machinery, Tools / Weapons, Wands and Traversal. Existing milestones
remain in a collapsible section; the HUD tracker remains optional.

- Expeditions use saved quest destinations. A journal with no selection tracks
  its first available unfinished quest. A completed tracked quest advances to
  another available quest. Explicit Untrack remains respected. Available
  objectives appear first; completed and unavailable entries are in Journal.
- Workshop readiness uses one shared rule for its journal and marker: an upgraded
  wand, owned melee weapon or armed assembled tool.
- Machinery routes show real recipes, ingredient stock, placed counts and the
  factory's current bottleneck. Names describe production routes, not invented
  research locks. Inputs open their recipes; Uses opens downstream recipes.
- Tool schemas list required part kinds from the tool table.
  Links open the matching Part Builder or assembly schema; casting links open
  the existing smeltery recipe and structure preview. Materials show mining tier.
  Part Builder displays the same voxel-scale-adjusted block cost used by payment.
- Wands shows the next Workbench recipe for the active tier and links to the
  Workbench and Wand Editor. Resource hovers show known drop species.
- Traversal recipes show equipped gear, current bindings and pack fuel.
- Recipe lookups from Projects leave a return link on Inventory, including after
  staging ingredients. Each Projects branch keeps its own scroll position.

Compilation does not establish native layout, quest-route reachability, completed
crafting, or co-op acceptance. No automated tests or hosted CI were requested.
