import test from "node:test";
import assert from "node:assert/strict";
import { auditRecipeGraph } from "./alchemy-cycles.mjs";
const recipe = (label, inputs, outputs) => ({ source: "fixture", label, inputs, outputs, ticks: 32, power: 8 });
const audit = rows => auditRecipeGraph(["A", "B", "FeedOrProduct"], rows);
test("finite external feed is not free production", () => {
  assert.equal(audit([recipe("feed", [[0,64],[2,64]], [[1,128]]), recipe("return", [[1,64]], [[0,64]])]).ok, true);
});
test("known closed gain is detected with full witness and costs", () => {
  const report = audit([recipe("grow", [[0,64]], [[1,128]]), recipe("return", [[1,64]], [[0,64]])]);
  assert.equal(report.ok, false);
  assert.ok(report.loops[0].witness.length > 0);
  assert.ok(report.loops[0].witness.every(w => w.energyUnits === w.workTicks * 8));
  assert.ok(report.loops[0].netResources.every(r => r.units >= 0));
});
test("balanced conversion is not gain", () => {
  assert.equal(audit([recipe("forward", [[0,64]], [[1,64]]), recipe("return", [[1,64]], [[0,64]])]).ok, true);
});
test("product outside SCC cannot hide a profitable cycle", () => {
  const report = audit([recipe("product", [[0,64]], [[1,64],[2,64]]), recipe("return", [[1,64]], [[0,64]])]);
  assert.equal(report.ok, false);
  assert.ok(report.loops[0].netResources.some(r => r.resource === "FeedOrProduct" && r.units > 0));
});
