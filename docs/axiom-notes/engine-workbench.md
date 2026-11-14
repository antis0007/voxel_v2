# Engine workbench

<!-- axiom-system-board -->

Humans and agents should inspect the same engine state and use the same bounded
live controls. Reduce context gathering and rebuild delays without confusing a
preview, source change, compiled build or observed gameplay result.

- [[tools/axiom-dev/ENGINE_WORKBENCH.md|Commands, capabilities and iteration guide]]
- [[crates/app/src/dev_panel/engine.rs|Native validation, revisions, inspection and undo]]
- [[tools/axiom-dev/engine-service.mjs|Shared local transport and experiment evidence]]
- [[tools/axiom-dev/engine-panel.mjs|Schema-driven human controls]]
- [[tools/axiom-dev/engine-cli.mjs|Compact agent controls]]
- [[tools/axiom-dev/agent-context.mjs|Pinned sources and native observations]]
- [[crates/app/src/mesh/models/voxel_asset.rs|Live factory model replacement]]
- [[crates/app/src/material_studio.rs|Transient material previews]]
- [[docs/design/LOCATION_MANA_AMBIENCE.md|First integrated gameplay use case]]

## Design critique

An unrestricted script console would make actions hard to discover and receipts
hard to compare. The native protocol instead names supported operations and
returns units, limits, owners, current state and explicit rebuild requirements.
The existing source/bundle workflow handles changes beyond live capabilities.

Editing source for every balance experiment caused avoidable build waits. Live
overrides solve that for ambience, recovery, terrain finish and factory models;
they have bounded history and clear automatically on world changes. Vitals,
camera and time are explicitly separate world-state actions.

Screenshots and prose alone miss numerical causes. Narrow inspection sections
and short observation series expose actual state at small output cost. Captures
wait for the PNG end marker, and context records binary identity and revisions.
Source freshness, a compiled shader, a native observation and a visual verdict
remain distinct evidence; none substitutes for the others.

UI controls consume the native schema. Both interfaces reject invalid values
before publishing and report stale revisions instead of silently overwriting an
experiment. Imported/static model support remains bounded to the implemented
factory voxel loader, with authored source and undo available throughout.

Native iteration exposed two fixture assumptions: a short automatic exit timer,
and scripted inventory/HUD/capture phases. Engine workbench factory sessions now
retain their long editing lifetime and leave UI and capture timing to the user.
Inspection follows the render position even in a fixed-camera scene. A terrain
survey provides bounded navigation leads; it never labels generated positions
as collision clearance or measured mana. See the linked location review for
native receipts and the remaining browser-review limitation.
