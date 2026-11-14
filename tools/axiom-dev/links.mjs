import { parse, decode, title } from "./syntax.mjs";

const file = name => "assets/" + name + ".ron";
const catalogNames = ["materials", "spells", "machines", "bestiary", "wands", "recipes",
  "spawns", "biomes", "environment", "trades", "reactions", "industry", "mining", "progression",
  "stations", "star_systems", "foods", "brewing", "crops", "panning", "fishing", "fernwake_trades",
  "advancements", "goals", "stacks", "tools/materials", "tools/parts", "tools/traits"];
const kindTarget = new Map([["Block", "materials"], ["Spell", "spells"], ["Enemy", "bestiary"],
  ["Machine", "machines"]]);
const fields = {
  block: "materials", block_id: "materials", surface_block: "materials", floor_block: "materials",
  spell: "spells", spell_id: "spells", spells: "spells", cards: "spells",
  enemy: "bestiary", enemy_kind: "bestiary", machine: "machines", machine_kind: "machines",
  biome: "biomes", biome_id: "biomes",
  a_into: "materials", b_into: "materials", melts_into: "materials", cools_into: "materials",
  boils_into: "materials", burns_into: "materials", solid: "materials", liquid: "materials",
};
const idFields = new Set(["id", "kind", "name", "display_name", "why"]);
const normalized = value => String(value).trim().toLowerCase();

function references(node, current, available, output = [], field) {
  const target = kindTarget.get(node.prefix) ?? fields[field ?? node.key];
  if (target && node.children?.length === 1 && node.children[0].raw !== undefined) {
    output.push({ path: file(target), key: normalized(decode(node.children[0].raw)) });
  }
  if (!node.children) {
    const raw = decode(node.raw);
    if (target) output.push({ path: file(target), key: normalized(raw) });
    if ((field ?? node.key) === "kind" && current === file("industry")) output.push({ path: file("machines"), key: normalized(raw) });
    if (available.has(raw) && raw.startsWith("assets/")) output.push({ path: raw, key: "" });
  } else {
    for (const child of node.children) {
      references(child, current, available, output, node.opener === "[" ? field ?? node.key : undefined);
    }
  }
  return output;
}

function records(root, current, available) {
  const output = [];
  function visit(node, at, label) {
    if (!node.children) return;
    const tuple = node.opener === "(" && !node.key && node.children[0]?.raw && node.children[1]?.children;
    const fields = tuple ? node.children[1].children : node.children;
    const ids = fields.filter(child => idFields.has(child.key) && child.raw).map(child => normalized(decode(child.raw)));
    if (tuple) ids.push(normalized(decode(node.children[0].raw)));
    if (ids.length) {
      output.push({ path: current, at, label, ids, refs: references(node, current, available) });
      return;
    }
    node.children.forEach((child, index) => visit(child, [...at, index], title(child, index)));
  }
  visit(root, [], current.split("/").at(-1));
  return output;
}

let cache;

export function clearLinks() { cache = undefined; }

export async function links(api, paths, current, root, selected, trail) {
  const available = new Set(paths);
  const loading = cache ??= Promise.all(catalogNames.map(async name => {
    const relative = file(name);
    if (!available.has(relative) || relative === current) return [];
    try {
      const doc = await api("/api/read", relative);
      return records(parse(doc.text, doc.kind), relative, available);
    } catch { return []; }
  })).then(groups => ({ except: current, entries: groups.flat() }));
  const loaded = await loading;
  let others = loaded.entries.filter(row => row.path !== current);
  if (loaded.except !== current && available.has(loaded.except) && loaded.except.endsWith(".ron")) {
    try {
      const doc = await api("/api/read", loaded.except);
      others = [...others, ...records(parse(doc.text, doc.kind), loaded.except, available)];
    } catch { /* An invalid catalog has no verified links. */ }
  }

  const here = records(root, current, available);
  const outgoing = new Map();
  for (const ref of references(selected, current, available)) {
    if (!available.has(ref.path)) continue;
    const row = [...here, ...others].find(row => row.path === ref.path && row.ids.includes(ref.key));
    const item = row ?? (ref.key ? null : { path: ref.path, at: [], label: ref.path.split("/").at(-1) });
    if (item) outgoing.set(item.path + "#" + item.at.join("."), item);
  }
  const selectedRecord = here.filter(row => row.at.every((index, offset) => trail[offset] === index))
    .sort((a, b) => b.at.length - a.at.length)[0];
  const incoming = selectedRecord ? [...here, ...others].filter(row => row !== selectedRecord
    && row.refs.some(ref => ref.path === current && selectedRecord.ids.includes(ref.key))) : [];
  return { outgoing: [...outgoing.values()], incoming };
}

