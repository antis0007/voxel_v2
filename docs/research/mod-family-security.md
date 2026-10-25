# Force fields and security

Scope: [Addendum A.7](../specs/MOD_ADDENDUM.md#a7-force-fields-and-security-voltz-era-mffs--modular-force-field-system-calclavias-security--electric-expansion-also-security-craft-opensecurity): MFFS, Calclavia's Security & Electric Expansion, Security Craft/SecurityCraft, OpenSecurity. Claims: [C-MC1/2/4](../specs/GAME_BIBLE.md#heavily-modded-minecraft-tech--magic--tinkers), [C-S6](../specs/GAME_BIBLE.md#neal-stephenson-snow-crash-the-diamond-age-cryptonomicon-anathem-seveneves-fall-the-baroque-cycles-natural-philosophy).

Primary evidence checked 2026-10-24: the [MFFS author's page](https://www.curseforge.com/minecraft/mc-mods/mffs) describes a projector, Fortron storage/generation, shape/size/offset upgrades and access control. [SecurityCraft's source](https://github.com/Geforce132/SecurityCraft) identifies its maintained implementation; exact camera/keypad behavior was not traced. Calclavia's other projects and OpenSecurity remain unverified.

Mapping proposal: projector-owned temporary cells, shared collision filters and energy accounting; readers/alarms are signal entities and logs are events. New projector/field lifecycle M4–M5; exotic modules M7. Local time scaling is especially unresolved because it crosses deterministic scheduler boundaries.

## What we adopt

Visible perimeter shapes, owner/identity permissions, configurable defensive effects and useful incident logs.

## What we reject

An indestructibility exception, orphaned field cells after projector loss, and claiming frequency matching is cryptography.

## What we improve

Bound field maintenance by dirty surfaces, make energy exhaustion and physical breaches readable, and route shock/liquid interactions through shared physics. Filter semantics and conservation remain acceptance targets.
