// Generator gate audit. A gate number must have one literal definition in
// code and a row in docs/worldgen/GENERATOR_GATES.md; every other name for it
// is an alias of that definition. Only `index --write` writes, and only the
// generated block of the registry.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { root } from "./workspace.mjs";

const registry = "docs/worldgen/GENERATOR_GATES.md";
// Rows start where numbering stopped being owned by one session (registry preface).
const firstRegistered = 54;
const literal = /^\s*pub(?:\([\w:\s]+\))?\s+const\s+(\w+):\s*u32\s*=\s*(\d+)\s*;/;
const compare = /generator(?:_version)?\s*(?:>=|<=|==|<|>)\s*\d+/g;

function* sources(dir) {
  for (const entry of readdirSync(path.join(root, dir), { withFileTypes: true })) {
    const rel = `${dir}/${entry.name}`;
    if (entry.isDirectory()) { if (entry.name !== "target") yield* sources(rel); }
    else if (entry.name.endsWith(".rs")) yield rel;
  }
}

// `*_GENERATOR` anywhere; a bare `VERSION` gate only inside the generator crate.
const gateName = (name, file) => /(^|_)GENERATOR$/.test(name)
  || (/^VERSION$|_VERSION$/.test(name) && file.startsWith("crates/worldgen/src/") && !/SAVE|FORMAT|CACHE|SNAPSHOT/.test(name));

function scan() {
  const gates = [], compares = {};
  for (const file of sources("crates")) {
    const tests = /(^|\/)(tests|benches|examples)\/|tests?\.rs$/.test(file);
    readFileSync(path.join(root, file), "utf8").split(/\r?\n/).forEach((text, index, lines) => {
      const found = literal.exec(text);
      if (found && gateName(found[1], file)) {
        // The doc comment directly above, when the constant has one.
        let first = index;
        while (first > 0 && /^\s*\/\/\//.test(lines[first - 1])) first--;
        const note = lines.slice(first, index).map(line => line.replace(/^\s*\/\/\/\s?/, "")).join(" ").trim();
        gates.push({ number: Number(found[2]), name: found[1], file, at: `${file}:${index + 1}`, note });
      }
      const hits = tests ? 0 : (text.match(compare) ?? []).length;
      if (hits) compares[file] = (compares[file] ?? 0) + hits;
    });
  }
  const rows = new Set();
  const text = readFileSync(path.join(root, registry), "utf8");
  for (const line of text.split(/\r?\n/)) {
    // A row's first cell holds one gate, a list (`151, 163`) or a range (`83-88`);
    // early claims are prose: `Gate 98 is claimed`, `124 is \`volcano::VERSION\``.
    const cell = /^\|\s*(\d[\d\s,–-]*?)\s*\|/.exec(line)?.[1];
    for (const part of cell?.split(",") ?? []) {
      const [low, high = low] = part.trim().split(/\s*[-–]\s*/).map(Number);
      for (let n = low; n <= high && high < 1000; n++) rows.add(n);
    }
    for (const prose of line.matchAll(/(?:\bGates? (\d+) is|^(\d+) is `)/g)) rows.add(Number(prose[1] ?? prose[2]));
  }
  return { gates, compares, rows, text };
}

const begin = "<!-- gates-cli index: begin (generated, do not edit) -->", end = "<!-- gates-cli index: end -->";
// One row per literal gate constant, from source. The hand-written tables above
// it carry claims, branches and evidence; this block cannot drift from code.
function indexBlock(gates) {
  const rows = [...gates].sort((a, b) => a.number - b.number || a.name.localeCompare(b.name))
    .map(g => `| ${g.number} | \`${g.name}\` | \`${g.file}\` | ${g.note.replaceAll("|", "/")} |`);
  return [begin, "| Gate | Constant | File | Source note |", "| --- | --- | --- | --- |", ...rows, end].join("\n");
}
function index(write) {
  const { gates, text } = scan();
  const block = indexBlock(gates), at = text.indexOf(begin), stop = text.indexOf(end);
  const current = at >= 0 && stop > at ? text.slice(at, stop + end.length) : "";
  const stale = current.replace(/\r\n/g, "\n") !== block;
  if (write && stale) {
    const eol = text.includes("\r\n") ? "\r\n" : "\n", body = block.replaceAll("\n", eol);
    writeFileSync(path.join(root, registry), current ? text.replace(current, () => body)
      : `${text.trimEnd()}${eol}${eol}## Gate constants in source${eol}${eol}${body}${eol}`);
  }
  return { rows: gates.length, stale: stale && !write, written: write && stale };
}

function check() {
  const { gates, compares, rows } = scan();
  const byNumber = Map.groupBy(gates, gate => gate.number);
  const duplicates = [...byNumber].filter(([, defs]) => defs.length > 1)
    .map(([number, defs]) => ({ number, definitions: defs.map(d => `${d.name} ${d.at}`) }));
  const unregistered = gates.filter(g => g.number >= firstRegistered && !rows.has(g.number))
    .map(g => ({ number: g.number, name: g.name, at: g.at }));
  const literals = Object.entries(compares).sort((a, b) => b[1] - a[1]);
  const stale_index = index(false).stale;
  return {
    ok: duplicates.length === 0 && unregistered.length === 0 && !stale_index,
    stale_index,
    gates: gates.length, registered_rows: rows.size,
    duplicates, unregistered,
    fix: "Keep one literal (shared numbers live in crates/worldgen/src/gates.rs); write the others as `pub const B: u32 = path::A;`. Stale or missing index rows: `gates-cli index --write`.",
    literal_compares: { total: literals.reduce((sum, [, n]) => sum + n, 0), files: literals.length, top: literals.slice(0, 8) },
  };
}

const [command, ...rest] = process.argv.slice(2);
const flag = name => { const i = rest.indexOf(name); return i < 0 ? undefined : rest[i + 1]; };
let result;
if (command === "check") result = check();
else if (command === "list") {
  const from = Number(flag("--from") ?? 0), limit = Number(flag("--limit") ?? 40);
  result = scan().gates.filter(g => g.number >= from).sort((a, b) => a.number - b.number).slice(0, limit);
} else if (command === "index") result = index(rest.includes("--write"));
else if (command === "next") {
  const { gates, rows } = scan();
  result = { next: Math.max(...gates.map(g => g.number), ...rows) + 1, claim_in: registry };
} else result = { commands: {
  check: "numbers with more than one literal definition, gate constants with no registry row, stale index, bare numeric compares",
  "index [--write]": "the generated table of gate constants in the registry: is it current; --write refreshes it",
  "list [--from N] [--limit 40]": "gate constants in number order",
  next: "first number above every constant and registry row" }, registry };
console.log(JSON.stringify(result));
if (command === "check" && !result.ok) process.exitCode = 1;
