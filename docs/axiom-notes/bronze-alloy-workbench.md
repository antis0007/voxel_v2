# Bronze alloy workbench

<!-- axiom-system-board -->

Tune a real existing alloy and its assembled-tool properties. Both this board and the alloy CLI use Forge's native-validated, revision-checked bundle publisher.

- [[assets/tools/materials.ron#r%3Aid%3Abronze/k%3Aprops/k%3Adurability|Bronze durability]]
- [[assets/tools/materials.ron#r%3Aid%3Abronze/k%3Aprops/k%3Amining_speed|Bronze mining speed]]
- [[assets/tools/materials.ron#r%3Aid%3Abronze/k%3Aprops/k%3Aattack|Bronze attack]]
- [[assets/tools/materials.ron#r%3Aid%3Abronze/k%3Aprops/k%3Aconductivity|Bronze conductivity]]
- [[assets/reactions.ron#r%3Abatch_product%3AMoltenBronze/k%3Abatch_counts/t%3A0/t%3A0|Copper volume count]]
- [[assets/reactions.ron#r%3Abatch_product%3AMoltenBronze/k%3Abatch_counts/t%3A0/t%3A1|Tin volume count]]

Use **Review system changes** to publish the two catalog drafts together. A rejected draft leaves both files untouched. Rebuild and relaunch Axiom to load saved definitions; this is not hot reload. The bronze kiln and connected molten smeltery use the same authored volume ratio.

The block, molten form, ingot, tool material and kiln mode retain their existing IDs. New block IDs, ore generation and new native effects need separate engine integration.

AI command interface: `node tools/axiom-dev/alloy-cli.mjs inspect bronze`, then send a request with `material`, `changes`, and `batchCounts` to `preview`. Apply the exact preview with its `versions` and `approval`. Prepare native checkers with `node tools/axiom-dev/build-validators.mjs`.
