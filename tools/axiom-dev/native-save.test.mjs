import { parse as parseRon } from "./syntax.mjs";
const parseFabrication = text => parseRon(text, "ron");
import test from "node:test";
import assert from "node:assert/strict";
import * as fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// Exercise the real native binaries and Workspace publication code in a copy.
// Build validators first with `node tools/axiom-dev/build-validators.mjs`.
test("native checks protect single saves, candidate bundles and restores", async t => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const fixtureBase = path.join(root, "target", "native-save-fixtures");
  fs.mkdirSync(fixtureBase, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(fixtureBase, "axiom-native-save-"));
  try {
    for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]) {
      if (fs.existsSync(path.join(root, file))) fs.copyFileSync(path.join(root, file), path.join(fixture, file));
    }
    for (const relative of ["assets", "tools/axiom-dev", "docs/axiom-notes", "crates/core", "crates/content", "crates/audio", "crates/tools",
      "target/axiom-dev/native-validation"]) {
      fs.cpSync(path.join(root, relative), path.join(fixture, relative), { recursive: true });
    }
    const moduleURL = file => pathToFileURL(path.join(fixture, "tools/axiom-dev", file));
    const { read, save, saveTextBundle, restoreTextBundle, files } = await import(moduleURL("workspace.mjs"));
    const spells = "assets/spells.ron", tools = "assets/tools/parts.ron";
    const originalSpell = read(spells), originalTools = read(tools);
    const validSpell = originalSpell.text.replace("cost: 12.0", "cost: 12.5");
    assert.notEqual(validSpell, originalSpell.text);
    const changedTools = originalTools.text.replace("blocking: false", "blocking: true");
    assert.notEqual(changedTools, originalTools.text);

    await t.test("semantically invalid spell is rejected without publishing or backing up", () => {
      assert.throws(() => save(spells, { version: originalSpell.version,
        text: originalSpell.text.replace("cost: 12.0", "cost: -12.0") }), /Native validation rejected.*cost/);
      assert.equal(read(spells).version, originalSpell.version);
      assert.deepEqual(read(spells).backups, originalSpell.backups);
    });
    await t.test("valid single edit saves with a backup and can restore through validation", () => {
      const next = save(spells, { version: originalSpell.version, text: validSpell });
      assert.equal(next.text, validSpell);
      const backup = next.backups.find(id => !originalSpell.backups.includes(id));
      assert.ok(backup);
      save(spells, { version: next.version, restore: backup });
      assert.equal(read(spells).version, originalSpell.version);
    });
    await t.test("one invalid catalog rejects the whole candidate bundle before publication", () => {
      const invalidTools = originalTools.text.replace("fells_trees: false", "fells_trees: true");
      assert.throws(() => saveTextBundle([
        { path: spells, version: originalSpell.version, text: validSpell },
        { path: tools, version: originalTools.version, text: invalidTools },
      ]), /Native validation rejected.*Cut/);
      assert.equal(read(spells).version, originalSpell.version);
      assert.equal(read(tools).version, originalTools.version);
    });
    await t.test("native schema and catalog cannot be published together against an old checker", () => {
      const source = "crates/tools/src/parts.rs", doc = read(source);
      assert.throws(() => saveTextBundle([
        { path: spells, version: originalSpell.version, text: validSpell },
        { path: source, version: doc.version, text: doc.text + "\n// new schema\n" },
      ]), /unpublished native schema/);
      assert.equal(read(spells).version, originalSpell.version);
      assert.equal(read(source).version, doc.version);
    });
    await t.test("persisted tool material identities cannot be silently renamed by a source edit", () => {
      const path = "assets/tools/materials.ron", doc = read(path);
      assert.throws(() => save(path, { version: doc.version,
        text: doc.text.replace('id: "bronze"', 'id: "renamed_bronze"') }), /IDs and row positions/);
      assert.equal(read(path).version, doc.version);
    });
    await t.test("Fabricator restores retain appended machine recipe identities", () => {
      const file = "assets/fabrication.ron", doc = read(file);
      const { children, close } = parseFabrication(doc.text);
      const row = children[0];
      const clone = doc.text.slice(row.start, row.end).replace(/id: 0/, "id: " + children.length)
        .replace(/name: "[^"]*"/, 'name: "Identity restore probe"');
      const next = save(file, { version: doc.version, text: doc.text.slice(0, close) + clone + ",\n" + doc.text.slice(close) });
      const backup = next.backups.find(id => !doc.backups.includes(id));
      assert.ok(backup);
      assert.throws(() => save(file, { version: next.version, restore: backup }), /IDs and row positions/);
      assert.equal(read(file).version, next.version);
    });
    let bundle;
    await t.test("valid bundle publishes the exact checked drafts", () => {
      saveTextBundle([
        { path: spells, version: originalSpell.version, text: validSpell },
        { path: tools, version: originalTools.version, text: changedTools },
      ], plans => { bundle = plans; });
      assert.equal(read(spells).text, validSpell);
      assert.equal(read(tools).text, changedTools);
    });
    await t.test("stale schemas block restore and leave the published bundle intact", () => {
      const file = path.join(fixture, "crates/tools/src/capabilities.rs");
      const source = fs.readFileSync(file);
      try {
        fs.appendFileSync(file, "\n// changed schema\n");
        assert.throws(() => restoreTextBundle(bundle), /missing or stale/);
        assert.equal(read(spells).text, validSpell);
        assert.equal(read(tools).text, changedTools);
      } finally { fs.writeFileSync(file, source); }
    });
    await t.test("validated bundle restore recovers both original catalogs", () => {
      restoreTextBundle(bundle);
      assert.equal(read(spells).version, originalSpell.version);
      assert.equal(read(tools).version, originalTools.version);
    });
    await t.test("missing validators block catalog writes but leave unrelated editing available", () => {
      const receipt = path.join(fixture, "target/axiom-dev/native-validation/receipt.json");
      fs.renameSync(receipt, receipt + ".held");
      try {
        assert.throws(() => save(spells, { version: originalSpell.version, text: validSpell }), /build-validators/);
        assert.equal(read(spells).version, originalSpell.version);
        const doc = read("Cargo.toml");
        save("Cargo.toml", { version: doc.version, text: doc.text + "\n# unrelated edit\n" });
        assert.equal(read("Cargo.toml").text, doc.text + "\n# unrelated edit\n");
        save("Cargo.toml", { version: read("Cargo.toml").version, text: doc.text });
      } finally { fs.renameSync(receipt + ".held", receipt); }
    });
    await t.test("alloy commands validate and publish real material and recipe edits as one reversible bundle", async () => {
      const { inspectAlloy, previewAlloy, applyAlloy } = await import(moduleURL("alloy-authoring.mjs"));
      const { revertBundle } = await import(moduleURL("agent-bundles.mjs"));
      const original = inspectAlloy("bronze");
      assert.equal(original.properties.durability, 430);
      assert.deepEqual(original.batchCounts, [3, 1]);
      assert.throws(() => previewAlloy({ material: "bronze", changes: { durability: 0 } }), /Native validation rejected/);
      assert.throws(() => previewAlloy({ material: "bronze", batchCounts: [0, 1] }), /Native validation rejected/);
      const request = { material: "bronze", changes: { durability: 480 }, batchCounts: [2, 1], versions: original.versions };
      const preview = previewAlloy(request);
      assert.equal(preview.files.length, 2);
      assert.deepEqual(inspectAlloy("bronze"), original, "preview must not write");
      const applied = applyAlloy({ ...request, approval: preview.approval });
      const after = inspectAlloy("bronze");
      assert.equal(after.properties.durability, 480);
      assert.deepEqual(after.batchCounts, [2, 1]);
      assert.throws(() => applyAlloy({ ...request, approval: preview.approval }), /changed on disk/);
      revertBundle(applied.bundle);
      assert.deepEqual(inspectAlloy("bronze"), original);
    });
    await t.test("the existing node links expose bronze block, molten recipe and valid board controls", async () => {
      const { parse } = await import(moduleURL("syntax.mjs"));
      const { links } = await import(moduleURL("links.mjs"));
      const { isSystemBoard, noteLinks, resolve } = await import(moduleURL("targets.mjs"));
      const paths = files().map(file => file.path);
      const materials = parse(read("assets/tools/materials.ron").text, "ron");
      const index = materials.children.findIndex(row => row.children.some(field => field.key === "id" && field.raw === '"bronze"'));
      const api = async (_, relative) => read(relative);
      const outgoing = await links(api, paths, "assets/tools/materials.ron", materials, materials.children[index], [index]);
      assert.ok(outgoing.outgoing.some(row => row.path === "assets/materials.ron"));
      const voxel = parse(read("assets/materials.ron").text, "ron");
      const molten = voxel.children.findIndex(row => row.children.some(field => field.key === "id" && field.raw === "MoltenBronze"));
      const related = await links(api, paths, "assets/materials.ron", voxel, voxel.children[molten], [molten]);
      assert.ok(related.incoming.some(row => row.path === "assets/reactions.ron"));
      const board = read("docs/axiom-notes/bronze-alloy-workbench.md").text;
      assert.ok(isSystemBoard(board));
      for (const pin of noteLinks(board)) assert.ok(resolve(parse(read(pin.path).text, "ron"), pin.parts)?.node);
    });
    await t.test("alloy pins follow the product after recipe reordering and reject ambiguous products", async () => {
      const { parse } = await import(moduleURL("syntax.mjs"));
      const { noteLinks, resolve, stable } = await import(moduleURL("targets.mjs"));
      const pins = noteLinks(read("docs/axiom-notes/bronze-alloy-workbench.md").text)
        .filter(pin => pin.path === "assets/reactions.ron");
      assert.equal(pins.length, 2);
      assert.ok(pins.every(pin => stable(pin.parts)));
      const source = read("assets/reactions.ron").text;
      const rows = parse(source, "ron").children.map(row => source.slice(row.start, row.end));
      const reordered = parse("[" + rows.toReversed().join(",") + "]", "ron");
      assert.deepEqual(pins.map(pin => Number(resolve(reordered, pin.parts)?.node.raw)), [3, 1]);
      const duplicate = parse("[" + rows[0] + "," + rows[0] + "]", "ron");
      assert.ok(pins.every(pin => resolve(duplicate, pin.parts) === null));
    });
  } finally {
    const resolved = fs.realpathSync(fixture);
    const base = fs.realpathSync(fixtureBase) + path.sep;
    if (!resolved.startsWith(base) || !path.basename(resolved).startsWith("axiom-native-save-")) {
      throw new Error("Refusing to remove unexpected test fixture directory");
    }
    fs.rmSync(resolved, { recursive: true, force: true });
  }
});
