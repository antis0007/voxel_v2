# Reference: FTL

Research date: 2026-10-24. Scope: A.1 vessels, expeditions and run tension. Proposed adaptations below are unimplemented C-F* obligations.

## Primary-source findings

Subset's own product description connects crew orders, power allocation, targeted systems, pausing, randomized encounters and permanent run loss. Its shield-failure example permits escape, increased weapons or boarding, demonstrating interacting resources rather than one prescribed response. [Developer-authored Steam description](https://store.steampowered.com/app/212680/FTL_Faster_Than_Light/)

FTL also persists ship unlocks and achievements; the developer FAQ discusses recovering those records. Consequently, the bible's "no persistence" criticism should mean absence of a continuously built shared physical world, not absence of all progression. [Developer FAQ](https://subsetgames.com/faq.html)

These sources do not prove autonomous utility-based crew repair, physically simulated hull stress or generated event rules: those are Underkingdom proposals. FTL's pause is useful evidence about readable decisions, not an authorization to change the project's fixed-tick or multiplayer policy.

## What we adopt

Expose a vessel as dependent subsystems with competing power, crew and repair demands. Make a resource spent on the current incident unavailable for the next encounter. Give expedition sites conditions that enable different choices according to vessel equipment, crew and cargo (C-F1, C-F2).

## What we reject

Avoid event choices whose consequences exist only in dialogue flags. Avoid separate airship, submarine and rover damage/crew implementations. Use common hull, compartment, network, jobs and body mechanisms, with each vessel's configuration supplying its differences (C-F1, C-F2).

## What we improve

Propose a breach changing compartment pressure and hazards; crew perceive the hazard, reserve seal/repair/evacuation jobs, and choose according to capability, access and urgency. Failed routes release reservations and permit evacuation without an encounter-specific script (C-F1). Site generation reads seed plus current factions, inventories and hazards; choosing a trade or rescue emits ordinary world actions and persists their effects (C-F2). Delve death leaves a body and damaged structures in the same persistent world while codex records preserve demonstrated knowledge (C-F3). Required evidence is a generic breach-response scenario, reproducible state-changing expeditions and a death/revisit scenario. Product choices about permadeath remain subject to the bible's escalation rule.
