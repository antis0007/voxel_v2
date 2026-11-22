// Whole-game graph: every catalog record is a node and every reference between
// records is an edge. Nothing here is hand-listed per catalog: which field
// points at which catalog is inferred from the data, so a new catalog or field
// joins the graph without a code change.
import * as fs from "node:fs";
import * as path from "node:path";
import { root, files, read, revision } from "./workspace.mjs";
import { parse, decode, title } from "./syntax.mjs";
import { resourceCatalog } from "./workshop-planning.mjs";

const idKeys = new Set(["id", "kind", "name", "display_name", "key"]);
const nameKeys = ["name", "display_name", "title", "label", "id", "kind", "key", "species", "why"];
// Numeric ids are too ambiguous to infer, so these few are named outright.
const numericFields = {
  spell: "spells", spell_id: "spells", spells: "spells", cards: "spells",
  enemy: "bestiary", enemy_kind: "bestiary", prey: "bestiary", "P:Spell": "spells", "P:Enemy": "bestiary",
};
// Factory stacks name a resource by its registered number.
const resourceFields = { processes: ["inputs", "output", "byproduct"], fabrication: ["inputs", "output"],
  machines: ["recipe", "accepts"], autocraft: ["inputs"] };
const tupleKeyTarget = { spawns: "bestiary", food_chain: "bestiary", mutations: "bestiary" };
const wrappers = new Set(["Some", "None"]);
const groups = [
  ["World", /biome|climate|planet|star_system|site|landmark|ruin|dungeon|sky|atmosphere|environment|ore_|deep_water|danger|z_level|portal|undercrown|wreck|vacuum|world_tier|legend|structure|discover/],
  ["Materials", /^materials$|reaction|fracture|kinetic|isotope|element|connected_tex|chisel|footstep|fire_style|impact|micro_part|conversion|transmute/],
  ["Magic", /spell|wand|rune|ritual|altar|aspect|aura|mana|taint|enchant|constellation|demon|elven|brewing|implant/],
  ["Creatures", /bestiary|spawn|burrow|food_chain|mutation|genetic|wildlife|hive|boss|entity|raid|incident/],
  ["Industry", /machine|process|fabricat|industry|autocraft|assembler|factory|drone|turtle|rail|cannon|turret|defense|converter|wireless|peripheral|chip|display|bucketwheel|emp|failure|utility|orbital|station|bodies|traversal|balance/],
  ["Crafting", /recipe|tools\/|mining|progression|resource|prep|food|crop|fishing|panning|stack|chest_loot/],
  ["Settlement", /city|fernwake|resident|trade|district|freight|gate_route|reputation|community|colony/],
  ["Player", /keybind|hud|emote|subtitle|goal|advancement|guidebook|primer|lesson|description|verb|avatar|viewmodel|music|session|multiplayer|keep_awake|system|workshop_fit|biome_sound/],
];

const norm = value => String(value).toLowerCase().replace(/[^a-z0-9]/g, "");
const scalar = node => node && node.raw !== undefined;
const catalogId = file => file.replace(/^assets\//, "").replace(/\.ron$/, "");
const pretty = text => String(text).replace(/_/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2")
  .replace(/^\w/, char => char.toUpperCase());

function isRecord(node) {
  if (!node.children?.length || !["(", "{"].includes(node.opener)) return false;
  if (node.children.some(child => child.key !== undefined)) return true;
  return node.children.length === 2 && scalar(node.children[0]) && node.children[1].children?.some(child => child.key !== undefined);
}

function isRecordList(node) {
  return node.children?.length > 0 && node.children.every(isRecord)
    && (node.opener === "[" || (node.opener === "{" && node.children.every(child => child.key !== undefined)));
}

function describe(node, index) {
  const body = !node.key && node.children[1]?.children && scalar(node.children[0]) ? node.children[1] : node;
  const named = nameKeys.map(key => body.children.find(child => child.key === key && scalar(child))).find(Boolean);
  if (named) return decode(named.raw);
  if (node.key !== undefined && node.opener !== "[") return node.key;
  const firsts = body.children.filter(scalar).slice(0, 2).map(child => decode(child.raw)).filter(Boolean);
  return "#" + (index + 1) + (firsts.length ? " " + firsts.join(" · ") : "");
}

function collect(file, tree) {
  const catalog = catalogId(file);
  const records = [];
  function leaves(node, context, via, record, lead) {
    if (scalar(node)) {
      const value = decode(node.raw);
      const key = node.key ?? context;
      if (key && value !== "" && !/^(true|false|None)$/.test(value) && !/^-?\d*\.\d/.test(value)) {
        record.refs.push({ key, via, value, lead });
      }
      return;
    }
    if (!node.children) return;
    const own = node.key ?? context;
    const wraps = node.prefix && !wrappers.has(node.prefix) && node.children.length === 1 && scalar(node.children[0]);
    if (isRecordList(node) && node.children.length > 1
      && node.children.every(child => child.children.some(field => idKeys.has(field.key) && scalar(field)))) {
      node.children.forEach((child, index) => record.nested.push({ node: child, index, list: node }));
      return;
    }
    const tuple = node.opener === "(" && !node.prefix && node.children.every(child => child.key === undefined);
    node.children.forEach((child, index) =>
      leaves(child, own, wraps ? "P:" + node.prefix : undefined, record, !tuple || index === 0));
  }
  function add(node, at, index, parent) {
    const tuple = !node.key && node.children[1]?.children && scalar(node.children[0]);
    const body = tuple ? node.children[1] : node;
    const record = { catalog, file, at, title: title(node, index), label: pretty(describe(node, index)).slice(0, 60),
      ids: new Set(), idents: new Set(), refs: [], nested: [], parent };
    const ident = value => { if (/^[A-Z][A-Za-z0-9]{4,}$/.test(value)) record.idents.add(value); };
    if (node.key !== undefined) ident(node.key);
    if (tuple) {
      const key = decode(node.children[0].raw);
      ident(key);
      record.ids.add(/^\d+$/.test(key) ? "#" + key : norm(key));
      if (tupleKeyTarget[catalog] && /^\d+$/.test(key)) record.refs.push({ key: "kind", value: key, numeric: tupleKeyTarget[catalog], lead: true });
    }
    if (node.key !== undefined) record.ids.add(norm(node.key));
    for (const field of body.children) {
      if (idKeys.has(field.key) && scalar(field)) {
        const value = decode(field.raw);
        record.ids.add(/^\d+$/.test(value) ? "#" + value : norm(value));
        if (field.key !== "name" && field.key !== "display_name") ident(value);
        if (field.key === "id") record.primary = /^\d+$/.test(value) ? "#" + value : norm(value);
      }
    }
    record.ids.delete("");
    for (const child of body.children) leaves(child, undefined, undefined, record, true);
    records.push(record);
    for (const item of record.nested) {
      const listTrail = [];
      (function find(current, trail) {
        if (current === item.list) { listTrail.push(...trail); return true; }
        return current.children?.some((child, offset) => find(child, [...trail, offset])) ?? false;
      })(node, []);
      add(item.node, [...at, ...listTrail, item.index], item.index, record);
    }
    record.nested = undefined;
  }
  function visit(node, at, depth) {
    if (!node.children) return;
    if (isRecordList(node)) return node.children.forEach((child, index) => add(child, [...at, index], index));
    if (depth < 3) node.children.forEach((child, index) => visit(child, [...at, index], depth + 1));
  }
  visit(tree, [], 0);
  return records;
}

// ---- code layer -----------------------------------------------------------
// Every Rust source file is a node. It links to the catalogs it reads, the
// modules it declares or uses, and the catalog records it names.
const commonWords = new Set(["Default", "Option", "String", "Result", "Other", "Normal", "Custom", "Empty", "Unknown"]);

function crateNames() {
  const names = new Map();
  for (const entry of files()) {
    const match = /^crates\/([^/]+)\/Cargo\.toml$/.exec(entry.path);
    if (!match) continue;
    try {
      const name = /^\s*name\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(path.join(root, entry.path), "utf8"))?.[1];
      if (name) names.set(name.replace(/-/g, "_"), match[1]);
    } catch { /* A crate without a readable manifest keeps only local links. */ }
  }
  return names;
}

function scanCode(catalogNames, idents) {
  const sources = files().map(entry => entry.path)
    .filter(file => /^crates\/[^/]+\/.*\.rs$/.test(file) && !/(?:_tests|\/tests)\.rs$|\/tests\/|\/benches\//.test(file));
  const known = new Set(sources);
  const crates = crateNames();
  const catalogSet = new Set(catalogNames);
  const resolve = (crate, segments) => {
    for (let length = segments.length; length > 0; length--) {
      const base = "crates/" + crate + "/src/" + segments.slice(0, length).join("/");
      if (known.has(base + ".rs")) return base + ".rs";
      if (known.has(base + "/mod.rs")) return base + "/mod.rs";
    }
    return null;
  };
  const rows = [];
  for (const file of sources) {
    let text;
    try { text = fs.readFileSync(path.join(root, file), "utf8"); } catch { continue; }
    const crate = file.split("/")[1];
    const row = { path: file, crate, lines: text.split("\n").length, reads: new Set(), links: new Map(), mentions: new Set() };
    if (text.includes(".ron")) {
      for (const match of text.matchAll(/([\w/]+)\.ron\b/g)) {
        const full = match[1].replace(/^.*assets\//, "");
        const name = catalogSet.has(full) ? full : catalogSet.has(full.split("/").at(-1)) ? full.split("/").at(-1) : null;
        if (name) row.reads.add(name);
      }
    }
    const parts = file.split("/");
    const name = parts.at(-1);
    const parent = parts.slice(0, -1).join("/");
    const nested = /^(lib|main|mod)\.rs$/.test(name) ? parent : parent + "/" + name.replace(/\.rs$/, "");
    for (const match of text.matchAll(/^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;/gm)) {
      const hit = [nested + "/" + match[1] + ".rs", nested + "/" + match[1] + "/mod.rs", parent + "/" + match[1] + ".rs"].find(item => known.has(item));
      if (hit && hit !== file) row.links.set(hit, "declares");
    }
    for (const match of text.matchAll(/\b([a-z_][a-z0-9_]*)::((?:[a-z_][a-z0-9_]*::)*[a-z_][a-z0-9_]*)/g)) {
      const owner = match[1] === "crate" ? crate : crates.get(match[1]);
      if (!owner) continue;
      const hit = resolve(owner, match[2].split("::")) ?? (owner !== crate && known.has("crates/" + owner + "/src/lib.rs") ? "crates/" + owner + "/src/lib.rs" : null);
      if (hit && hit !== file && !row.links.has(hit)) row.links.set(hit, "uses");
    }
    for (const match of text.matchAll(/\b[A-Z][A-Za-z0-9]{4,}\b/g)) if (idents.has(match[0])) row.mentions.add(match[0]);
    rows.push(row);
  }
  // Group files into modules: crate/top, split once more when a module is large.
  const keyOf = (row, deep) => {
    const inner = row.path.split("/").slice(2);
    if (inner[0] !== "src") return row.crate + "/(build)";
    const rest = inner.slice(1);
    const top = rest.length === 1 ? "(root)" : rest[0];
    if (!deep || rest.length < 2) return row.crate + "/" + top;
    return row.crate + "/" + top + "/" + (rest.length === 2 ? "(root)" : rest[1]);
  };
  const sizes = new Map();
  for (const row of rows) sizes.set(keyOf(row, false), (sizes.get(keyOf(row, false)) ?? 0) + 1);
  for (const row of rows) row.module = keyOf(row, sizes.get(keyOf(row, false)) > 70);
  return rows;
}

// ---- multiplayer coverage -------------------------------------------------
// Which pieces of shared world state reach a joining player. Read from the
// `World` struct and from what the multiplayer sources touch, so a new system
// shows up as unsynced until replication names it. `net-coverage.json` holds
// reviewed verdicts for fields the scan cannot judge.
function netCoverage() {
  const read = file => { try { return fs.readFileSync(path.join(root, file), "utf8"); } catch { return ""; } };
  const world = read("crates/app/src/world.rs");
  const start = world.search(/pub struct World\s*\{/);
  if (start < 0) return [];
  const body = world.slice(start, world.indexOf("\n}", start));
  let notes = {};
  try { notes = JSON.parse(read("tools/axiom-dev/net-coverage.json") || "{}"); } catch { /* Unreadable notes leave the raw scan. */ }
  const sources = files().map(entry => entry.path).filter(file =>
    /^crates\/app\/src\/multiplayer(\.rs|\/)/.test(file) && !/tests?\.rs$|_review\.rs$|\/tests\//.test(file));
  const texts = sources.map(file => [file, read(file)]);
  const rows = [];
  for (const match of body.matchAll(/^\s*pub\s+([a-z_]+):\s*([A-Za-z_:]+(?:<[^\n]*>)?)\s*,/gm)) {
    const [, field, type] = match;
    const segments = type.replace(/<.*$/, "").split("::");
    if (segments.length < 2 || /^(std|engine_voxel|Option|Arc|Vec)$/.test(segments[0])) continue;
    const inner = segments.slice(0, -1);
    const module = inner[0] === "crate" ? inner.slice(1).join("/") : "world/" + inner.join("/");
    const pattern = new RegExp("\\b(?:w|world|self\\.world|target|source)\\." + field + "\\b");
    const hits = texts.filter(([, text]) => pattern.test(text)).map(([file]) => file);
    const status = hits.some(file => /\/replication|\/gameplay\.rs|\/replica|\/deltas\.rs/.test(file)) ? "replicated"
      : hits.length ? "handled" : "local";
    rows.push({ field, type, module, status, files: hits.slice(0, 8), note: notes[field] });
  }
  return rows;
}

let cached, codeCache;

function codeStamp() {
  if (codeCache && Date.now() - codeCache.checked < 15000) return codeCache.stamp;
  let total = 0, count = 0;
  for (const entry of files()) {
    if (!/^crates\/[^/]+\/.*\.rs$/.test(entry.path)) continue;
    try { total += fs.statSync(path.join(root, entry.path)).mtimeMs; count++; } catch { /* Removed since the index was built. */ }
  }
  const stamp = count + ":" + Math.round(total);
  codeCache = { ...(codeCache?.stamp === stamp ? codeCache : {}), stamp, checked: Date.now() };
  return stamp;
}

export function buildGameGraph() {
  const sources = files().filter(entry => /^assets\/(?:tools\/)?[^/]+\.ron$/.test(entry.path)).map(entry => entry.path);
  const stamp = sources.map(file => { try { return revision(file); } catch { return ""; } }).join("|") + "|" + codeStamp();
  if (cached?.stamp === stamp) return cached.graph;

  const catalogs = [];
  const records = [];
  for (const file of sources) {
    const id = catalogId(file);
    const catalog = { id, path: file, label: pretty(id.startsWith("tools/") ? "tool " + id.slice(6) : id),
      group: groups.find(([, test]) => test.test(id))?.[0] ?? "Settings", count: 0, error: undefined };
    try {
      const doc = read(file);
      const found = collect(file, parse(doc.text, doc.kind));
      catalog.count = found.length;
      records.push(...found);
    } catch (error) { catalog.error = String(error.message).slice(0, 160); }
    catalogs.push(catalog);
  }
  const catalogIndex = new Map(catalogs.map((catalog, index) => [catalog.id, index]));
  const ids = new Map(catalogs.map(catalog => [catalog.id, new Map()]));
  const issues = [];
  records.forEach((record, index) => {
    record.index = index;
    for (const id of record.ids) {
      const known = ids.get(record.catalog);
      if (!known.has(id)) known.set(id, index);
      else if (id === record.primary && records[known.get(id)].primary === id && records[known.get(id)].parent === record.parent) {
        issues.push({ type: "duplicate", node: index, text: "Same id as another " + record.catalog + " record: " + record.label });
      }
    }
  });

  // Infer which (catalog, field) pairs point at which catalog.
  const usage = new Map();
  for (const record of records) {
    for (const ref of record.refs) {
      if (ref.key === "audio") continue;
      if (ref.numeric || /^\d+$/.test(ref.value)) continue;
      const slot = record.catalog + "\n" + (ref.via ?? ref.key);
      if (!usage.has(slot)) usage.set(slot, []);
      usage.get(slot).push(norm(ref.value));
    }
  }
  const schema = new Map();
  for (const [slot, values] of usage) {
    const [source, key] = slot.split("\n");
    const usable = values.filter(value => value.length > 1);
    const accepted = [];
    let covered = new Set();
    for (const [target, known] of ids) {
      if (!known.size) continue;
      if (target === source && idKeys.has(key)) continue;
      const hits = usable.filter(value => known.has(value));
      const distinct = new Set(hits).size;
      const rate = hits.length / Math.max(1, usable.length);
      const strict = idKeys.has(key) || key === "label" || key === "title";
      if (distinct >= 2 && rate >= (strict || target === source ? .7 : .3)) {
        accepted.push({ target, hits: hits.length });
        covered = new Set([...covered, ...hits]);
      }
    }
    if (accepted.length) {
      const matched = usable.filter(value => covered.has(value)).length;
      schema.set(slot, { targets: accepted.sort((a, b) => b.hits - a.hits).map(row => row.target),
        firm: usable.length >= 8 && matched / usable.length >= .85 && !idKeys.has(key) });
    }
  }

  const labels = [];
  const labelIndex = new Map();
  const labelOf = text => {
    if (!labelIndex.has(text)) { labelIndex.set(text, labels.length); labels.push(text); }
    return labelIndex.get(text);
  };
  const edges = new Map();
  const link = (from, to, label) => {
    if (from === to) return;
    const key = from + ">" + to;
    const known = edges.get(key);
    if (!known) edges.set(key, [from, to, labelOf(label)]);
    else if (!labels[known[2]].split(", ").includes(label) && labels[known[2]].length < 40) {
      known[2] = labelOf(labels[known[2]] + ", " + label);
    }
  };
  const resourceRows = new Map();
  try {
    for (const { id, name } of resourceCatalog()) {
      const hit = ids.get("resources")?.get(norm(name));
      if (hit !== undefined) resourceRows.set(String(id), hit);
    }
  } catch { /* Resource numbers stay unlinked without the contract. */ }
  for (const record of records) {
    if (record.parent) link(record.parent.index, record.index, "contains");
    for (const ref of record.refs) {
      const slot = ref.via ?? ref.key;
      const label = pretty(ref.key).toLowerCase();
      if (/^\d+$/.test(ref.value)) {
        if (!ref.lead) continue;
        if (slot === "P:Resource" || resourceFields[record.catalog]?.includes(ref.key)) {
          const hit = resourceRows.get(ref.value);
          if (hit !== undefined) link(record.index, hit, label);
          continue;
        }
        const target = ref.numeric ?? numericFields[slot] ?? numericFields[ref.key];
        const hit = target && ids.get(target)?.get("#" + ref.value);
        if (hit !== undefined) link(record.index, hit, label);
        continue;
      }
      const value = norm(ref.value);
      const named = numericFields[slot] ?? numericFields[ref.key];
      const rule = schema.get(record.catalog + "\n" + slot);
      let linked = false;
      for (const target of new Set([...(rule?.targets ?? []), ...(named ? [named] : [])])) {
        const hit = ids.get(target)?.get(value);
        if (hit !== undefined) { link(record.index, hit, label); linked = true; }
      }
      if (!linked && rule?.firm && value.length > 1) {
        issues.push({ type: "dangling", node: record.index,
          text: record.label + " · " + label + " = " + ref.value + " matches nothing in " + rule.targets[0] });
      }
    }
  }

  // Nodes: records, then one file node per catalog, then code files.
  const nodes = records.map(record => ({ c: catalogIndex.get(record.catalog), label: record.label, title: record.title, at: record.at,
    key: [...record.idents][0] }));
  const hubs = catalogs.map((catalog, index) => {
    nodes.push({ c: index, label: catalog.path.split("/").at(-1), hub: true });
    return nodes.length - 1;
  });
  const idents = new Map();
  for (const record of records) for (const ident of record.idents) {
    if (commonWords.has(ident)) continue;
    if (!idents.has(ident)) idents.set(ident, []);
    idents.get(ident).push(record.index);
  }
  if (codeCache.rows === undefined || codeCache.identCount !== idents.size) {
    codeCache.rows = scanCode(catalogs.map(catalog => catalog.id), idents);
    codeCache.identCount = idents.size;
  }
  const rows = codeCache.rows;
  const modules = new Map();
  const fileNode = new Map();
  for (const row of rows) {
    if (!modules.has(row.module)) {
      const crate = row.crate;
      modules.set(row.module, catalogs.length);
      catalogs.push({ id: "code:" + row.module, kind: "code", label: row.module.split("/").slice(1).join("/").replace("(root)", "root files") || row.module,
        path: "crates/" + crate + "/src" + (row.module.split("/").slice(1).filter(part => !part.startsWith("(")).map(part => "/" + part).join("")),
        group: crate === "app" ? "Code · app" : crate === "worldgen" ? "Code · worldgen" : "Code · engine", count: 0, readers: [] });
      if (!["app", "worldgen"].includes(crate)) catalogs.at(-1).label = crate + " · " + catalogs.at(-1).label;
    }
    const home = catalogs[modules.get(row.module)];
    home.count++;
    fileNode.set(row.path, nodes.length);
    nodes.push({ c: modules.get(row.module), label: row.path.split("/").slice(-2).join("/").replace(/^src\//, ""), file: row.path, lines: row.lines });
  }
  for (const catalog of catalogs) catalog.readers ??= [];
  const mentions = new Map();
  for (const row of rows) for (const ident of row.mentions) mentions.set(ident, (mentions.get(ident) ?? 0) + 1);
  const named = new Uint32Array(records.length);
  for (const row of rows) {
    const from = fileNode.get(row.path);
    for (const name of row.reads) {
      link(from, hubs[catalogIndex.get(name)], "reads");
      catalogs[catalogIndex.get(name)].readers.push(row.path);
    }
    for (const [target, label] of row.links) link(from, fileNode.get(target), label);
    for (const ident of row.mentions) {
      for (const record of idents.get(ident)) {
        named[record] += 1;
        if (mentions.get(ident) <= 60) link(from, record, "names");
      }
    }
  }
  records.forEach((record, index) => { if (named[index]) nodes[index].code = named[index]; });
  for (const catalog of catalogs) {
    catalog.readers ??= [];
    if (catalog.kind !== "code" && !catalog.readers.length) issues.push({ type: "unread", catalog: catalogIndex.get(catalog.id), text: catalog.path + " is not named by any Rust source" });
    catalog.readers = catalog.readers.slice(0, 40);
  }
  const incoming = new Uint32Array(nodes.length);
  for (const [, to] of edges.values()) incoming[to]++;
  const byCatalog = new Map();
  for (const record of records) {
    if (!byCatalog.has(record.catalog)) byCatalog.set(record.catalog, []);
    byCatalog.get(record.catalog).push(record.index);
  }
  for (const [name, members] of byCatalog) {
    const live = members.filter(index => incoming[index] || named[index]);
    if (members.length < 6 || live.length / members.length < .6) continue;
    for (const index of members) {
      if (!incoming[index] && !named[index]) issues.push({ type: "unused", node: index, text: "Nothing refers to " + records[index].label + ", unlike most " + name + " records" });
    }
  }

  const net = netCoverage();
  for (const row of net) {
    const base = "crates/app/src/" + row.module;
    let any = false;
    for (const [file, index] of fileNode) {
      if (file === base + ".rs" || file.startsWith(base + "/")) { nodes[index].net = row.status; any = true; }
    }
    row.node = fileNode.get(base + ".rs") ?? fileNode.get(base + "/mod.rs") ?? (any ? [...fileNode].find(([file]) => file.startsWith(base + "/"))?.[1] : undefined);
    if (row.status === "local" && !row.note?.ok) {
      issues.push({ type: "unsynced", node: row.node, text: "World." + row.field + " (" + row.type + ") has no path to joining players"
        + (row.note ? " · " + row.note.why : "") });
    }
  }

  const graph = {
    catalogs, nodes, edges: [...edges.values()], labels, issues, net,
    stamp: String(stamp.length) + ":" + [...stamp].reduce((sum, char) => (sum * 31 + char.charCodeAt(0)) >>> 0, 7),
    schema: [...schema].map(([slot, rule]) => ({ from: slot.split("\n")[0], key: slot.split("\n")[1], to: rule.targets, firm: rule.firm })),
  };
  cached = { stamp, graph };
  return graph;
}
