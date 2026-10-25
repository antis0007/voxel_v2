# Reference: heavily modded Minecraft

Research date: 2026-10-24. Scope: A.1 design comparison; mod-family notes supply individual feature evidence. These are reference observations and proposed mechanisms, not completed C-* claims.

## Primary-source findings

- Minecraft's redstone connects placed components into circuits and automated builds; spatial arrangement is itself a programming surface. [Mojang's redstone introduction](https://www.minecraft.net/en-us/creator/article/redstone)
- Mekanism describes tiered factories and energy storage, with longer processing chains improving ore yield. This supports a progression graph, not a universal physical energy model. [Developer repository](https://github.com/mekanism/Mekanism)
- Botania explicitly describes itself as a technology mod themed around natural magic: separate presentation need not mean separate semantics. [Developer repository](https://github.com/VazkiiMods/Botania)
- Tinkers' Construct 3 assembles tools from parts, casts molten materials, creates alloys, and supplies an in-game guide adapting to pack materials. [Developer gameplay overview](https://slimeknights.github.io/docs/gameplay/tinkers-construct-3/)

**Qualification:** the bible's blanket criticism that mods lack interaction or consequences is a design diagnosis, not established for every modpack. Existing integrations and contextual hazards must be evaluated per version. This note does not establish that RF values are intrinsically arbitrary or that every mod duplicates a concept.

## What we adopt

Make machines, transport and controller connections readable world objects; retain many recipes as data over a small mechanism set. Assemble tool heads, handles and armour components from material properties, with repair and replacement preserving player investment (C-MC3). Use an in-world guide and local demonstrations as the entry to automation (C-MC5, C-S1).

## What we reject

Do not create one independent simulation per reference mod. Distinct recipes, appearances and progression roles can share processing, transport, storage and power mechanisms (C-MC4). Do not make external documentation a prerequisite for the first automated resource (C-MC5).

## What we improve

Propose one energy ledger with explicit converters and dissipated heat; one hazard-response contract with visible damage and repair; one material-to-equipment derivation path. Acceptance remains pending: converter-cycle accounting (C-MC1), hazard-by-machine coverage (C-MC2), a ten-property RON-only material addition (C-MC3), mechanism ownership lint (C-MC4), and a two-hour hint-only onboarding scenario (C-MC5). The closed-loop check must account for stored energy and external inputs as well as losses; a low measured efficiency alone does not prove conservation.
