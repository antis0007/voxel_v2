# Hand and wand controls

- Ctrl + wheel or [ / ] adjusts reach from 2 to 24 metres. Alt + wheel changes size; ordinary wheel changes material slots.
- B opens compact reach and size settings. Q is a focused tool picker and closes after choosing a tool.
- The compact corner card shows size, reach, target distance, queued edits and the hand placement material budget. Beyond-reach feedback stays at the target and corner card.
- Hand movement uses a gentle single-cycle gait, with no absolute-value vertical bouncing. Confirmed edits trigger completed action strokes; asynchronous edits cannot restart a stroke mid-animation.
- Normal building blocks preserve 64 cubic metres per inventory slot: 64 at 1 m, 512 at 0.5 m, 4096 at 0.25 m. Model objects, equipment, food and already volume-normalized resources retain their limits.
- Inventory Drop creates a recoverable world stack. F retrieves a nearby stack, preserving leftovers when the inventory is full. Save version 16 preserves dropped stacks and scaled inventories.
- Generator 12 prismatic caverns include a broken elevated road, scale-aligned approach steps, an inscription and an elevated survey cache. Building, climbing and excavation remain valid approaches. Historical generator versions retain their terrain.

## Verification

Focused local checks passed for all three stack scales through insertion, split/merge, transfer, encode/decode, dropping and retrieval; full save/load of large stacks and dropped stacks plus legacy v12 migration; and real-generator bridge deck/gap/cache presence at all three voxel sizes. Native acceptance is recorded separately after desktop review.

Optional native review environment: VOXEL_TOOL_REVIEW logs confirmed edits and inventory retrieval. VOXEL_REVIEW_CAUSEWAY starts newly created non-flat review worlds at a naturally generated causeway without changing its terrain. Normal launches do not enable these options.
