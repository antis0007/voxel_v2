import test from "node:test";
import assert from "node:assert/strict";
import { renderingEvidence } from "./foundry-agent-context.mjs";

test("legacy evidence never implies full GPU coverage or shader identity", () => {
  const r = renderingEvidence({telemetry:{gpu_coverage:{unique_samples:100}}});
  assert.equal(r.gpu_timing_scope.kind, "unknown");
  assert.equal(r.gpu_timing_scope.complete_frame_cost, false);
  assert.equal(r.shader_validation.status, "unavailable");
  assert.deepEqual(r.shader_validation.records, []);
});

test("captured scope and identities survive AI context unchanged", () => {
  const scope = {kind:"partial_pass_sum",complete_frame_cost:false};
  const shader = {status:"reported",records:[{program:"world",build_match:false}]};
  const r = renderingEvidence({telemetry:{gpu_coverage:{timing_scope:scope}, gpu_ms:{gpu_sky_probe_ms:{p50:0.25}}},
    auxiliary:{shader_validation:shader}});
  assert.strictEqual(r.gpu_timing_scope, scope);
  assert.equal(r.sky_probe_compute_ms.p50, 0.25);
  assert.strictEqual(r.shader_validation, shader);
});
