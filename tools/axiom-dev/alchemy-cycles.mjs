// Conversion-cycle audit (parity rule C-MC1: no profitable conversion loop).
// Read-only. Reads processes.ron, fabrication.ron, brewing.ron and resources.ron
// from disk; nothing is duplicated here. Quantities are item units (64 per item),
// not mass. Results apply only to the parsed recipe model, not all runtime paths.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = file => readFileSync(root + file, "utf8").replace(/^﻿/, "").replace(/\/\/[^\n]*/g, "");
const pair = text => text.match(/\(\s*(\d+)\s*,\s*(\d+)\s*\)/g)?.map(p => p.match(/\d+/g).map(Number)) ?? [];

export function resourceNames() {
  return [...read("assets/resources.ron").matchAll(/name:\s*"([^"]*)"/g)].map(m => m[1]);
}

/** One recipe: {source, label, inputs:[[res,qty]], outputs:[[res,qty]], external:bool} */
export function loadRecipes() {
  const recipes = [];
  const rows = text => text.split("\n").filter(line => /^\s*\(/.test(line));
  for (const line of rows(read("assets/processes.ron"))) {
    const machine = line.match(/machine:\s*(\w+)/)?.[1];
    const recipe = line.match(/recipe:\s*(\d+)/)?.[1];
    const inputs = pair(line.match(/inputs:\s*\[([^\]]*)\]/)?.[1] ?? "");
    const output = pair(line.match(/output:\s*(\(\s*\d+\s*,\s*\d+\s*\))/)?.[1] ?? "");
    const byproduct = pair(line.match(/byproduct:\s*Some\(([^)]*\))\)/)?.[1] ?? "");
    if (!machine || !output.length) continue;
    recipes.push({
      source: "processes", label: `${machine}#${recipe}`, inputs,
      outputs: [...output, ...byproduct],
      block: line.match(/block:\s*Some\(\s*"([^"]+)"\s*\)/)?.[1] ?? null,
      ticks: Number(line.match(/ticks:\s*(\d+)/)?.[1]),
      power: Number(line.match(/power:\s*(\d+)/)?.[1]),
    });
  }
  for (const line of rows(read("assets/fabrication.ron"))) {
    const name = line.match(/name:\s*"([^"]*)"/)?.[1];
    recipes.push({
      source: "fabrication", label: `Fabricator:${name}`,
      inputs: pair(line.match(/inputs:\s*\[([^\]]*)\]/)?.[1] ?? ""),
      outputs: pair(line.match(/output:\s*(\(\s*\d+\s*,\s*\d+\s*\))/)?.[1] ?? ""),
      ticks: Number(line.match(/ticks:\s*(\d+)/)?.[1]),
      power: Number(line.match(/power:\s*(\d+)/)?.[1]),
    });
  }
  return recipes;
}

/** Brewing consumes a resource and yields a potion (a sink, never a resource). */
export function loadBrewing() {
  return [...read("assets/brewing.ron").matchAll(/base:\s*(\w+)\((\w+)\)[^)]*?modifier:\s*(\w+)\((\w+)\)[^)]*?output:\s*(\w+)/gs)]
    .map(m => ({ base: `${m[1]}(${m[2]})`, modifier: `${m[3]}(${m[4]})`, output: m[5] }));
}

function sccs(nodes, edges) {
  const adj = new Map(nodes.map(n => [n, []]));
  for (const [a, b] of edges) adj.get(a)?.push(b);
  let index = 0; const idx = new Map(), low = new Map(), on = new Set(), stack = [], out = [];
  const visit = v => {
    idx.set(v, index); low.set(v, index++); stack.push(v); on.add(v);
    for (const w of adj.get(v)) {
      if (!idx.has(w)) { visit(w); low.set(v, Math.min(low.get(v), low.get(w))); }
      else if (on.has(w)) low.set(v, Math.min(low.get(v), idx.get(w)));
    }
    if (low.get(v) === idx.get(v)) {
      const comp = []; let w;
      do { w = stack.pop(); on.delete(w); comp.push(w); } while (w !== v);
      out.push(comp);
    }
  };
  for (const n of nodes) if (!idx.has(n)) visit(n);
  return out;
}

/** Max of c.x over A x <= 0, sum x <= 1, x >= 0 (dense simplex, Bland's rule). */
function maximise(c, A) {
  const m = A.length + 1, n = c.length;
  const t = [];
  for (let i = 0; i < A.length; i++) t.push([...A[i], ...Array.from({ length: m }, (_, j) => (j === i ? 1 : 0)), 0]);
  t.push([...Array(n).fill(1), ...Array.from({ length: m }, (_, j) => (j === m - 1 ? 1 : 0)), 1]);
  t.push([...c.map(v => -v), ...Array(m).fill(0), 0]);
  const basis = Array.from({ length: m }, (_, i) => n + i), eps = 1e-9;
  for (let guard = 0; guard < 10000; guard++) {
    const col = t[m].findIndex((v, j) => j < n + m && v < -eps);
    if (col < 0) break;
    let row = -1, best = Infinity;
    for (let i = 0; i < m; i++) if (t[i][col] > eps) {
      const ratio = t[i][n + m] / t[i][col];
      if (ratio < best - eps || (Math.abs(ratio - best) <= eps && basis[i] < basis[row])) { best = ratio; row = i; }
    }
    if (row < 0) break;
    const p = t[row][col];
    t[row] = t[row].map(v => v / p);
    for (let i = 0; i <= m; i++) if (i !== row && t[i][col]) { const f = t[i][col]; t[i] = t[i].map((v, j) => v - f * t[row][j]); }
    basis[row] = col;
  }
  const x = Array(n).fill(0);
  basis.forEach((b, i) => { if (b < n) x[b] = t[i][n + m]; });
  return { value: t[m][n + m], x };
}

export function auditRecipeGraph(names, recipes, brewing = []) {
  const nodes = [...names.keys()];
  const edges = [];
  for (const r of recipes) for (const [i] of r.inputs) for (const [o] of r.outputs) edges.push([i, o]);
  const loops = [];
  for (const comp of sccs(nodes, edges)) {
    const set = new Set(comp);
    const inside = recipes.filter(r => r.outputs.some(([o]) => set.has(o)) && r.inputs.some(([i]) => set.has(i)));
    if (!inside.length || (comp.length === 1 && !inside.some(r => r.inputs.some(([i]) => i === comp[0])))) continue;
    // Constrain every resource, not only SCC members. Finite external feed
    // cannot be treated as free; products outside the SCC also count.
    const net = nodes.map(res => inside.map(r =>
      r.outputs.filter(([o]) => o === res).reduce((s, [, q]) => s + q, 0) - r.inputs.filter(([i]) => i === res).reduce((s, [, q]) => s + q, 0)));
    // Process::block_cost consumes one cubic metre per completed batch.
    // Block quantities are separate constraints, never added to item units.
    // No block-producing recipes are modelled, so block feed is finite here.
    const blockMaterials = [...new Set(inside.map(r => r.block).filter(Boolean))];
    const blockCosts = blockMaterials.map(material => inside.map(r => r.block === material ? 1 : 0));
    const { value, x } = maximise(inside.map((_, k) => net.reduce((s, row) => s + row[k], 0)),
      [...net.map(row => row.map(v => -v)), ...blockCosts]);
    const external = new Set();
    inside.forEach((r, k) => { if (x[k] > 1e-9) for (const [i, q] of r.inputs) if (!set.has(i)) external.add(names[i]); });
    loops.push({
      resources: comp.map(i => names[i]), recipes: inside.map(r => r.label),
      profitable: value > 1e-6, netGainPerUnitRun: +value.toFixed(3),
      activeRecipes: inside.filter((_, k) => x[k] > 1e-9).map(r => r.label),
      externalInputs: [...external],
      witness: inside.flatMap((r, k) => x[k] > 1e-9 ? [{
        recipe: r.label, batches: x[k],
        inputs: r.inputs, outputs: r.outputs,
        workTicks: Number.isFinite(r.ticks) ? r.ticks * x[k] : null,
        energyUnits: Number.isFinite(r.ticks) && Number.isFinite(r.power) ? r.ticks * r.power * x[k] : null,
        blockInput: r.block ? { material: r.block, cubicMetres: x[k] } : null,
      }] : []),
      netResources: net.map((row, i) => ({resource: names[i], units: row.reduce((sum, n, k) => sum + n * x[k], 0)}))
        .filter(row => Math.abs(row.units) > 1e-7),
    });
  }
  return {
    rule: "C-MC1 closed resource-cycle audit of parsed rows; 64 units/item, not mass",
    complete: false,
    limitations: ["Runtime catalyst handling, water, heat, upgrades and block-producing paths are not fully represented; this is not a runtime conservation proof.", "Cultivator mode 0 retains its biomass seed at runtime; parsed inputs currently model it as consumed.", "Witness work ticks are aggregate machine work, not elapsed time; energy is runtime power times work ticks. Declared process block feed is constrained separately in cubic metres."],
    modelled: { recipes: recipes.length, processes: recipes.filter(r => r.source === "processes").length,
      fabrication: recipes.filter(r => r.source === "fabrication").length, brewing: brewing.length },
    notModelled: ["reactions.ron (including the runtime bronze-process adapter)", "Touch of gold (hard-coded in adventure/spells.rs)",
      "mana flowers (wild essence -> mana)", "mass and mana conservation (needs aspects/mana units)"],
    loops, ok: !loops.some(l => l.profitable),
  };
}

export function auditCycles() {
  return auditRecipeGraph(resourceNames(), loadRecipes(), loadBrewing());
}
