# Location mana and ambience

<!-- axiom-system-board -->

Natural places are recovery destinations. Healthy mana pressure stays below
poisoning while providing a large regeneration bonus. Location, nearby plants,
water, enclosure, time of day and weather choose the visible and audible effects.

- [[crates/app/src/world/ambience.rs#rust%3ANATURAL_DENSITY_MAX|Natural mana ceiling]]
- [[crates/app/src/world/ambience.rs#rust%3AMANA_RECOVERY_MAX|Maximum natural mana regeneration]]
- [[crates/app/src/world/ambience.rs#rust%3AHEALTH_RECOVERY_MAX|Maximum natural health regeneration]]
- [[crates/app/src/world/ambience.rs#rust%3ATRANSITION_SPEED|Ambience transition speed]]
- [[crates/app/src/world/ambience.rs|Location profiles and recovery curves]]
- [[crates/app/src/world/adventure/affinity.rs#rust%3APOISON_AT|Toxic pressure threshold]]
- [[crates/app/src/soundscape/ambience.rs|Actual plants, canopy, water and enclosure]]
- [[crates/app/src/shaders/mood_shared.wgsl|Living air shapes and motion]]
- [[docs/design/LOCATION_MANA_AMBIENCE.md|Design, critique and native evidence]]
- [[tools/axiom-dev/ENGINE_WORKBENCH.md|Live controls, inspection, captures and presets]]

Use Engine workbench for live experiments; rebuild for changes to source. Recovery values are multipliers, density is
pressure, and the two scales are intentionally separate. Keep the natural ceiling
below the toxic threshold. Health still requires food and the existing hurt gate.
