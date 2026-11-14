import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { materialRows, miningSchemas, harvestGates, compareHarvest, reviseMiningLevel } from "./material-progression-data.mjs";
const read = path => readFileSync(fileURLToPath(new URL("../../" + path, import.meta.url)), "utf8");
const original = read("assets/tools/materials.ron"), parts = read("assets/tools/parts.ron");
const mining = read("assets/mining.ron"), blocks = read("assets/materials.ron");
const gates = harvestGates(mining, blocks), schemas = miningSchemas(parts);
const pickaxe = schemas.find(row => row.id === "pickaxe");

test("actual harvest rules include explicit gates and default-tier ore families", () => {
  assert.equal(gates.find(row => row.block === "IronOre").tier, 2);
  assert.equal(gates.find(row => row.block === "CopperOre").tier, 0);
  assert.equal(gates.find(row => row.block === "Cassiterite").deposit, "Tin");
  assert.match(gates.find(row => row.block === "CopperOre").source, /default/);
});
test("tier two gains iron, gold quartz and crystal but not platinum or mithril", () => {
  const impact = compareHarvest(gates, pickaxe, 1, 2), gained = impact.gained.map(row => row.block);
  for (const block of ["IronOre", "Hematite", "Magnetite", "GoldQuartz", "Crystal"]) assert(gained.includes(block));
  for (const block of ["PlatinumBlock", "MithrilBlock", "CopperOre"]) assert(!gained.includes(block));
  assert(impact.blocked.some(row => row.block === "MithrilBlock"));
  assert.deepEqual(compareHarvest(gates, pickaxe, 2, 1).lost.map(row => row.block), gained);
});
test("wrong mining class and schemas without tier parts gain no ore access", () => {
  const axe = schemas.find(row => row.id === "axe");
  assert(!compareHarvest(gates, axe, 0, 255).gained.some(row => row.block === "IronOre"));
  const noLevel = { ...pickaxe, levelParts: [] };
  assert.equal(compareHarvest(gates, noLevel, 1, 255).materialAffectsMiningLevel, false);
  assert.equal(compareHarvest(gates, noLevel, 1, 255).gained.length, 0);
  assert.equal(compareHarvest(gates, { ...pickaxe, miningClass: null }, 1, 255).gained.length, 0);
});
test("mining-level edit changes one scalar and preserves every material identity", () => {
  const before = materialRows(original), copper = before.find(row => row.id === "copper");
  const result = reviseMiningLevel(original, "copper", copper.miningLevel + 1);
  const after = materialRows(result.text);
  assert.deepEqual(after.map(row => row.id), before.map(row => row.id));
  assert.deepEqual(after.filter(row => row.id !== "copper").map(({ node, ...row }) => row), before.filter(row => row.id !== "copper").map(({ node, ...row }) => row));
  assert.equal(result.text.length, original.length);
  assert.equal([...original].filter((char, i) => char !== result.text[i]).length, 1);
  assert.equal(reviseMiningLevel(original, "copper", copper.miningLevel).text, original);
});
test("level authoring refuses malformed inputs instead of rounding or renaming", () => {
  for (const value of [-1, 256, 1.5, "2", null, NaN, Infinity]) assert.throws(() => reviseMiningLevel(original, "copper", value));
  assert.throws(() => reviseMiningLevel(original, "invented", 2));
  assert.equal(reviseMiningLevel(original, "copper", 255).after, 255);
});
test("ambiguous or unknown harvest definitions fail closed", () => {
  assert.throws(() => harvestGates(mining.replace("IronOre,", "NotAnOre,"), blocks), /Unknown/);
  assert.throws(() => harvestGates(mining.replace("IronOre,", "IronOre, IronOre,"), blocks), /duplicate/);
  assert.throws(() => harvestGates(mining.replaceAll("tier: 2,", "tier: 300,"), blocks));
  assert.throws(() => miningSchemas(parts.replace("class: Dig,", "class: Invented,")), /Unknown/);
});
