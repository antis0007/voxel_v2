---
paths:
  - "crates/app/src/world/inventory.rs"
  - "crates/app/src/world/inventory/**"
---
# Crafting transfers

- Withdraw every recipe input before publishing any destination grid cell. `remove_item` searches the craft grid before the cursor; publishing a partial recipe can consume an earlier staged cell instead of the held stack.
- Check cursor-only and split cursor/backpack/hotbar inputs with a repeated ingredient. Assert the selected recipe and batch count, not only a successful staging return. Keep shaped, shapeless, rejection, and multiplayer replay cases in `crafting_staging_tests.rs`.
