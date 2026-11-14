import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { catalogContracts, catalogContract, describeCatalog, nativeCatalogs, validatorBuilds } from "./catalog-contracts.mjs";
import { nativeSourcePaths, validateNativeBundle, validateStableCatalogIds } from "./native-content-validation.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = file => readFileSync(new URL("../../" + file, import.meta.url), "utf8");

test("catalog contracts have unique IDs, paths and native checker ownership", () => {
  for (const key of ["id", "path"]) assert.equal(new Set(catalogContracts.map(row => row[key])).size, catalogContracts.length);
  assert.equal(new Set(validatorBuilds.map(([, example]) => example)).size, catalogContracts.length);
  for (const row of catalogContracts) {
    assert.equal(describeCatalog(row.id), catalogContract(row.path));
    assert.equal(nativeCatalogs[row.path], row.validator.example);
    assert.ok(validatorBuilds.some(([crate, example]) => crate === row.validator.crate && example === row.validator.example));
    assert.equal(row.activation, "rebuild");
  }
});

test("registered paths resolve to the real native reader, schema and checker", () => {
  for (const row of catalogContracts) {
    assert.ok(read(row.path).length);
    assert.ok(read(row.schema).length);
    assert.ok(read(row.reader).includes(row.path));
    const crateDir = row.schema.split("/").slice(0, 2).join("/");
    assert.ok(read(crateDir + "/Cargo.toml").includes('name = "' + row.validator.crate + '"'));
    assert.ok(read(crateDir + "/examples/" + row.validator.example + ".rs").includes(row.path));
  }
});

test("contracts and derived consumer maps cannot be mutated", () => {
  assert.throws(() => catalogContracts.push({}), TypeError);
  assert.throws(() => catalogContracts[0].validator.example = "unchecked", TypeError);
  assert.throws(() => catalogContract("tool-parts").stableIdSections.push("anything"), TypeError);
  assert.throws(() => validatorBuilds[0][0] = "wrong", TypeError);
  assert.throws(() => nativeCatalogs["assets/spells.ron"] = "wrong", TypeError);
  assert.equal(catalogContract("assets/not-registered.ron"), undefined);
  assert.throws(() => describeCatalog("assets/not-registered.ron"), /Unregistered/);
});

test("stable-ID policy preserves saved identity while allowing append and value edits", () => {
  const before = '[(id: "a", value: 1), (id: "b", value: 2)]';
  for (const row of catalogContracts.filter(row => row.stableIdSections.length)) {
    for (const section of row.stableIdSections) {
      const wrap = data => section ? '(' + row.stableIdSections.map(key => key + ': ' + (key === section ? data : before)).join(', ') + ')' : data;
      const apply = data => validateStableCatalogIds(row.path, wrap(before), wrap(data));
      apply(before.replace("value: 1", "value: 8"));
      apply(before.replace(/]$/, ', (id: "c", value: 3)]'));
      for (const invalid of ['[(id: "b", value: 2), (id: "a", value: 1)]', '[(id: "a", value: 1)]', before.replace('"a"', '"renamed"')]) {
        assert.throws(() => apply(invalid), /IDs and row positions/);
      }
    }
  }
});

test("contract changes are validator dependencies and cannot publish with checked catalogs", () => {
  const file = "tools/axiom-dev/catalog-contracts.mjs";
  assert.ok(nativeSourcePaths(root).includes(file));
  const change = relative => ({ relative, original: Buffer.from("before"), next: Buffer.from("after") });
  assert.throws(() => validateNativeBundle(root, [change(file), change(catalogContracts[0].path)]), /unpublished native schema/);
});

test("discovery CLI is read-only JSON with explicit coverage and unknown-target failure", () => {
  const run = args => spawnSync(process.execPath, [fileURLToPath(new URL("catalog-cli.mjs", import.meta.url)), ...args], { encoding: "utf8", windowsHide: true });
  const listed = run(["list"]);
  assert.equal(listed.status, 0);
  assert.deepEqual(JSON.parse(listed.stdout).catalogs, catalogContracts);
  assert.match(JSON.parse(listed.stdout).scope, /only/);
  const detail = run(["describe", "tool-materials"]);
  assert.equal(detail.status, 0);
  assert.deepEqual(JSON.parse(detail.stdout), describeCatalog("tool-materials"));
  for (const args of [["describe", "unknown"], ["describe"], ["list", "extra"], ["delete", "spells"]]) {
    assert.equal(run(args).status, 1);
  }
});
