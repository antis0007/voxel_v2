import { after, test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, copyFileSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const base = mkdtempSync(path.join(tmpdir(), "forge-content-context-"));
after(() => {
  assert(path.resolve(base).startsWith(path.resolve(tmpdir()) + path.sep));
  rmSync(base, { recursive: true, force: true });
});
const toolPath = path.join(base, "tools", "axiom-dev");
mkdirSync(toolPath, { recursive: true });
for (const entry of readdirSync(fileURLToPath(new URL(".", import.meta.url)))) {
  if (entry.endsWith(".mjs")) copyFileSync(new URL(entry, import.meta.url), path.join(toolPath, entry));
}
const put = (file, text) => {
  mkdirSync(path.dirname(path.join(base, file)), { recursive: true });
  writeFileSync(path.join(base, file), text);
};
mkdirSync(path.join(base, "docs"));
const catalog = "assets/meal_values.ron";
const owner = "crates/content/src/registry.rs";
const original = Array.from({ length: 45 }, () => "// unrelated declaration").join("\n")
  + '\nconst DATA: &str = include_str!("../../../assets/meal_values.ron");\n'
  + 'fn decode() { parse(DATA); }\n';
put(catalog, "[(id: 0, nutrition: 4)]\n");
put(owner, original);
put("crates/content/src/different.rs", 'const OTHER: &str = include_str!("../../../assets/archive/meal_values.ron");\n');
put("crates/content/src/prefix.rs", 'const OTHER: &str = include_str!("../../../assets/meal_values.ron.backup");\n');
put("crates/content/src/comment.rs", '// include_str!("../../../assets/meal_values.ron")\n');
const { collectContext, assertContextFresh } = await import(pathToFileURL(path.join(toolPath, "agent-context.mjs")).href);
const request = { task: "Tune nutrition choices", paths: [catalog] };

test("catalog context follows the exact native asset reference without shared task terms", async () => {
  const packet = await collectContext(request);
  const hit = packet.evidence.find(row => row.path === owner && row.kind === "Content path reference");
  assert(hit, "retain the native embedding location even when its filename and text do not match the brief");
  assert.equal(hit.asset, catalog);
  assert.equal(hit.line, 46);
  assert.equal(hit.repairEligible, false, "following a content reference must not expand automatic repair ownership");
  assert.match(hit.excerpt, /include_str!/);
  assert.equal(hit.version.length, 64);
  assert(packet.evidence.length <= 12);
});

test("same basename, path prefixes and comment-only mentions do not become content references", async () => {
  const packet = await collectContext(request);
  for (const file of ["different", "prefix", "comment"]) {
    assert(!packet.evidence.some(row => row.path.endsWith(file + ".rs") && row.kind === "Content path reference"), file);
  }
});

test("catalog context becomes stale when its native reader changes", async () => {
  const packet = await collectContext(request);
  put(owner, original + "// native reader changed\n");
  try { assert.throws(() => assertContextFresh(packet), /changed on disk/); }
  finally { put(owner, original); }
});

test("following a target-only catalog and a multiline raw string retains provenance", async () => {
  const second = "assets/drink_values.ron", loader = "crates/content/src/fluid_registry.rs";
  put(second, "[(id: 0, hydration: 2)]\n");
  put(loader, 'const DATA: &str = include_str!(\n    r#"../../../assets/drink_values.ron"#\n);\n');
  const packet = await collectContext({ task: "Adjust hydration", targets: [{ path: second, line: 1 }] });
  assert(packet.evidence.some(row => row.path === loader && row.asset === second && row.kind === "Content path reference"));
});

test("native catalog links do not replace an explicit editable source target", async () => {
  const packet = await collectContext({ ...request, paths: [catalog, owner], targets: [{ path: owner, line: 46 }] });
  assert(packet.evidence.some(row => row.path === owner && row.kind === "Attached source" && row.repairEligible));
});

test("tooling-only tasks do not acquire native catalog references", async () => {
  const packet = await collectContext({ task: "Tune nutrition choices", paths: ["tools/axiom-dev/agent-context.mjs"] });
  assert(!packet.evidence.some(row => row.kind === "Content path reference"));
});

test("five catalogs retain one native reference each within the shared context budget", async () => {
  const paths = Array.from({ length: 5 }, (_, i) => "assets/choices_" + i + ".ron");
  for (const [i, catalog] of paths.entries()) {
    put(catalog, "[(id: 0)]\n");
    for (const suffix of ["a", "b"]) put("crates/content/src/choices_" + i + suffix + ".rs",
      'const DATA: &str = include_str!("../../../' + catalog + '");\n');
  }
  const packet = await collectContext({ task: "Tune variety", paths });
  for (const catalog of paths) {
    assert(packet.evidence.some(row => row.path === catalog), catalog + " source");
    assert(packet.evidence.some(row => row.asset === catalog), catalog + " native reference");
  }
  assert(packet.evidence.length <= 12);
});

test("an unavailable content search is reported without claiming complete coverage", async () => {
  const savedPath = process.env.PATH;
  process.env.PATH = base;
  try {
    const packet = await collectContext(request);
    assert(packet.warnings.some(row => row.includes("Content reference search incomplete")));
    assert(packet.evidence.some(row => row.path === catalog));
    assert(!packet.evidence.some(row => row.kind === "Content path reference"));
  } finally { process.env.PATH = savedPath; }
});

test("lexical noise cannot crowd attached catalogs and their native references out of sampling", async () => {
  for (let i = 0; i < 40; i++) put("crates/noise_" + i + ".rs", "// crafting process inputs output\n".repeat(3));
  const packet = await collectContext({ task: "Tune crafting process inputs and output", paths: [catalog] });
  assert(packet.evidence.some(row => row.path === catalog), "retain the selected catalog before lexical matches");
  assert(packet.evidence.some(row => row.path === owner && row.asset === catalog), "retain the native catalog reference before lexical matches");
  assert(packet.evidence.length <= 12);
  assert(packet.sampledFiles <= 26);
});

test("client targets cannot supply internal reference provenance or evidence kinds", async () => {
  const packet = await collectContext({ task: "Inspect registry", targets: [{ path: owner, line: 46,
    kind: "Existing test source", asset: "assets/invented.ron", readOnly: true }] });
  const hit = packet.evidence.find(row => row.path === owner && row.kind === "Attached source");
  assert(hit);
  assert.equal(hit.asset, undefined);
  assert.equal(hit.repairEligible, true);
});


test("registered catalogs include their shared ownership contract as read-only evidence", async () => {
  put("assets/tools/materials.ron", '[(id: "bronze", value: 1)]');
  const packet = await collectContext({ task: "Review bronze material balance", paths: ["assets/tools/materials.ron"] });
  const contract = packet.evidence.find(row => row.kind === "Catalog ownership contract");
  assert.ok(contract);
  assert.equal(contract.path, "tools/axiom-dev/catalog-contracts.mjs");
  assert.equal(contract.asset, "assets/tools/materials.ron");
  assert.equal(contract.repairEligible, false);
  assert.match(contract.excerpt, /check_tool_materials/);
  assert.match(contract.excerpt, /stableIdSections/);
  assert.match(contract.excerpt, /activation: "rebuild"/);
  assertContextFresh(packet);
  const original = readFileSync(path.join(toolPath, "catalog-contracts.mjs"), "utf8");
  try {
    put("tools/axiom-dev/catalog-contracts.mjs", original + "\n// contract revision\n");
    assert.throws(() => assertContextFresh(packet), /Context changed/);
  } finally { put("tools/axiom-dev/catalog-contracts.mjs", original); }
});
