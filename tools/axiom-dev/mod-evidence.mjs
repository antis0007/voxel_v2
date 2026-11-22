import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { mkdtemp, rm, rmdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { inflateRawSync } from "node:zlib";
import { pngDimensions } from "./pixel-grid.mjs";
import { voxelPrototype } from "./mod-model.mjs";

const modelEntry = /^assets\/[^/]+\/(?:models|geo)\/.+\.json$/i;

export const maxJarBytes = 48 * 1024 * 1024;
const maxEntryBytes = 256 * 1024;
const maxBundledBytes = 4 * 1024 * 1024;
const metadata = /^(?:fabric\.mod\.json|quilt\.mod\.json|mcmod\.info|META-INF\/(?:neoforge\.mods\.toml|mods\.toml|MANIFEST\.MF))$/i;
const hook = /registr|recipe|blockentit|tileentit|inventory|energy|mana|tick|fluid|network|packet|loot|worldgen|mixin|capabilit|event/i;
const display = text => text.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, "");

function directory(bytes, byteLimit = maxJarBytes) {
  if (bytes.length < 22 || bytes.length > byteLimit) throw new Error("ZIP size is outside the intake limit");
  let end = bytes.length - 22;
  const earliest = Math.max(0, end - 65535);
  for (; end >= earliest; end--) {
    if (bytes.readUInt32LE(end) === 0x06054b50 && end + 22 + bytes.readUInt16LE(end + 20) === bytes.length) break;
  }
  if (end < earliest) throw new Error("JAR has no ZIP directory");
  const count = bytes.readUInt16LE(end + 10);
  let at = bytes.readUInt32LE(end + 16);
  const limit = at + bytes.readUInt32LE(end + 12);
  if (bytes.readUInt32LE(end + 4) !== 0 || bytes.readUInt16LE(end + 8) !== count
      || count > 20000 || limit > end) throw new Error("Unsupported split, ZIP64, or oversized JAR directory");
  const entries = [];
  const seen = new Set();
  for (let i = 0; i < count; i++) {
    if (at + 46 > limit || bytes.readUInt32LE(at) !== 0x02014b50) throw new Error("Invalid JAR directory entry");
    const length = bytes.readUInt16LE(at + 28);
    const next = at + 46 + length + bytes.readUInt16LE(at + 30) + bytes.readUInt16LE(at + 32);
    if (next > limit) throw new Error("Truncated JAR directory");
    const name = display(bytes.toString("utf8", at + 46, at + 46 + length));
    if (name.length <= 400 && !name.endsWith("/") && !seen.has(name)) {
      entries.push({ name, flags: bytes.readUInt16LE(at + 8), method: bytes.readUInt16LE(at + 10),
        compressed: bytes.readUInt32LE(at + 20), size: bytes.readUInt32LE(at + 24), offset: bytes.readUInt32LE(at + 42) });
      seen.add(name);
    }
    at = next;
  }
  return entries;
}

function readEntry(bytes, entry, limit = maxEntryBytes) {
  const { offset, flags, method, size, compressed } = entry;
  if (flags & 1 || size > limit || compressed > limit || offset + 30 > bytes.length
      || bytes.readUInt32LE(offset) !== 0x04034b50) return;
  const nameEnd = offset + 30 + bytes.readUInt16LE(offset + 26);
  if (nameEnd > bytes.length || display(bytes.toString("utf8", offset + 30, nameEnd)) !== entry.name
      || bytes.readUInt16LE(offset + 8) !== method) return;
  const start = nameEnd + bytes.readUInt16LE(offset + 28);
  if (start + compressed > bytes.length) return;
  const input = bytes.subarray(start, start + compressed);
  const result = method === 0 ? input : method === 8 ? inflateRawSync(input, { maxOutputLength: limit }) : null;
  if (result?.length === size) return result;
}

function instructionLength(code, pc) {
  const op = code[pc];
  if (op === 0xaa || op === 0xab) {
    const aligned = (pc + 4) & ~3;
    const count = op === 0xaa ? code.readInt32BE(aligned + 8) - code.readInt32BE(aligned + 4) + 1
      : code.readInt32BE(aligned + 4);
    if (count < 0) return 0;
    return aligned - pc + (op === 0xaa ? 12 + count * 4 : 8 + count * 8);
  }
  if (op === 0xc4) return code[pc + 1] === 0x84 ? 6 : 4;
  if (op === 0xb9 || op === 0xba || op === 0xc8 || op === 0xc9) return 5;
  if (op === 0xc5) return 4;
  if ([0x11, 0x13, 0x14, 0x84, 0xbb, 0xbd, 0xc0, 0xc1, 0xc6, 0xc7].includes(op)
      || (op >= 0x99 && op <= 0xa8) || (op >= 0xb2 && op <= 0xb8)) return 3;
  if ([0x10, 0x12, 0xa9, 0xbc].includes(op) || (op >= 0x15 && op <= 0x19) || (op >= 0x36 && op <= 0x3a)) return 2;
  return 1;
}

function inspectBytecode(bytes) {
  if (!bytes || bytes.length < 10 || bytes.readUInt32BE(0) !== 0xcafebabe) return;
  let at = 8;
  const u8 = () => bytes.readUInt8(at++);
  const u16 = () => { const value = bytes.readUInt16BE(at); at += 2; return value; };
  const u32 = () => { const value = bytes.readUInt32BE(at); at += 4; return value; };
  const size = u16();
  const pool = new Array(size);
  for (let i = 1; i < size; i++) {
    const tag = u8();
    switch (tag) {
      case 1: {
        const length = u16();
        pool[i] = display(bytes.toString("utf8", at, at + length));
        at += length;
        break;
      }
      case 3: case 4: at += 4; break;
      case 5: case 6: at += 8; i++; break;
      case 7: case 8: case 16: case 19: case 20: pool[i] = { tag, a: u16() }; break;
      case 9: case 10: case 11: case 12: case 17: case 18: pool[i] = { tag, a: u16(), b: u16() }; break;
      case 15: pool[i] = { tag, kind: u8(), a: u16() }; break;
      default: return;
    }
    if (at > bytes.length) return;
  }
  const symbol = ref => {
    if (!ref || ![9, 10, 11].includes(ref.tag)) return;
    const owner = pool[pool[ref.a]?.a];
    const member = pool[pool[ref.b]?.a];
    const descriptor = pool[pool[ref.b]?.b];
    if (typeof owner === "string" && typeof member === "string") return {
      owner: owner.slice(0, 400), member: member.slice(0, 120),
      descriptor: typeof descriptor === "string" ? descriptor.slice(0, 240) : undefined,
    };
  };
  const className = index => pool[pool[index]?.a];
  at += 2;
  const owner = className(u16()), parent = className(u16());
  const interfaces = u16();
  const implementsNames = Array.from({ length: interfaces }, (_, i) => className(bytes.readUInt16BE(at + i * 2)));
  at += interfaces * 2;
  const membersByMethod = [];
  for (const methods of [false, true]) {
    const members = u16();
    for (let i = 0; i < members; i++) {
      at += 2;
      const method = String(pool[u16()] ?? "").slice(0, 80);
      const descriptor = pool[u16()];
      const attributes = u16();
      for (let j = 0; j < attributes; j++) {
        const kind = pool[u16()];
        const length = u32();
        const end = at + length;
        if (end > bytes.length) return;
        if (methods && kind === "Code" && length >= 8 && membersByMethod.length < 256) {
          const size = bytes.readUInt32BE(at + 4);
          if (size <= length - 8) {
            const code = bytes.subarray(at + 8, at + 8 + size);
            const row = { method, descriptor, bytes: size, edges: [], dynamic: [], strings: [], allocations: 0, backEdges: 0 };
            for (let pc = 0; pc < code.length;) {
              const width = instructionLength(code, pc);
              if (width <= 0 || pc + width > code.length) break;
              const op = code[pc];
              if (op >= 0xb2 && op <= 0xb9 && row.edges.length < 64) {
                const target = symbol(pool[code.readUInt16BE(pc + 1)]);
                if (target) row.edges.push({ pc, kind: op <= 0xb5 ? "field" : "call", ...target });
              } else if (op === 0xba && row.dynamic.length < 32) {
                const entry = pool[code.readUInt16BE(pc + 1)];
                if (entry?.tag === 18) row.dynamic.push({ pc, bootstrap: entry.a });
              } else if ((op === 0x12 || op === 0x13) && row.strings.length < 24) {
                const constant = pool[op === 0x12 ? code[pc + 1] : code.readUInt16BE(pc + 1)];
                const value = constant?.tag === 8 && pool[constant.a];
                if (typeof value === "string" && /^[a-z0-9_./:-]{2,96}$/i.test(value)) row.strings.push(value);
              }
              if ([0xbb, 0xbc, 0xbd, 0xc5].includes(op)) row.allocations++;
              if (((op >= 0x99 && op <= 0xa8) || op === 0xc6 || op === 0xc7) && code.readInt16BE(pc + 1) < 0) row.backEdges++;
              if ((op === 0xc8 || op === 0xc9) && code.readInt32BE(pc + 1) < 0) row.backEdges++;
              pc += width;
            }
            membersByMethod.push(row);
          }
        }
        at = end;
      }
    }
  }
  const attributes = u16();
  const bootstrap = [];
  for (let i = 0; i < attributes; i++) {
    const name = pool[u16()], length = u32(), end = at + length;
    if (end > bytes.length) return;
    if (name === "BootstrapMethods") {
      const count = u16();
      for (let n = 0; n < count && at + 4 <= end; n++) {
        const factory = symbol(pool[pool[u16()]?.a]), args = u16();
        if (at + args * 2 > end) return;
        const callbacks = [];
        for (let arg = 0; arg < args; arg++) {
          const handle = pool[u16()];
          if (handle?.tag === 15 && factory?.owner === "java/lang/invoke/LambdaMetafactory") {
            const target = symbol(pool[handle.a]);
            if (target && callbacks.length < 4) callbacks.push(target);
          }
        }
        bootstrap.push(callbacks);
      }
    }
    at = end;
  }
  for (const row of membersByMethod) {
    for (const { pc, bootstrap: index } of row.dynamic) {
      for (const target of bootstrap[index] ?? []) row.edges.push({ pc, kind: "lambda target", ...target });
    }
  }
  return { owner, parent, implementsNames, methods: membersByMethod,
    loaderMarker: pool.some(value => typeof value === "string" && /\/(?:fml\/common\/Mod|SubscribeEvent|asm\/mixin\/Mixin);$/.test(value)) };
}

function classLinks(bytes, focused = false) {
  const info = inspectBytecode(bytes);
  if (!info) return [];
  return info.methods.flatMap(row => {
    const costs = focused && (row.allocations || row.backEdges)
      ? [row.method + " · " + row.allocations + " allocation sites · " + row.backEdges + " backward branches"] : [];
    return [...costs, ...row.edges.filter(edge => focused || hook.test(row.method + "/" + edge.owner + "." + edge.member))
      .map(edge => (row.method + " @" + edge.pc + " " + edge.kind + " → " + edge.owner + "." + edge.member).slice(0, 200))];
  }).slice(0, focused ? 120 : 9);
}

function declaredHooks(entries, read) {
  const roots = new Map();
  const add = (value, reason) => {
    const name = (typeof value === "string" ? value : value?.value)?.split("::")[0]?.replaceAll(".", "/");
    if (typeof name === "string" && /^[\w$/-]{1,240}$/.test(name) && roots.size < 48) roots.set(name, reason);
  };
  for (const entry of entries.filter(row => /(?:fabric\.mod|quilt\.mod|mixins?[^/]*|[^/]*\.mixins?)\.json$/i.test(row.name)).slice(0, 24)) {
    try {
      const json = JSON.parse(read(entry)?.toString("utf8") ?? "null");
      const points = json?.entrypoints ?? json?.quilt_loader?.entrypoints ?? {};
      for (const [kind, values] of Object.entries(points)) {
        for (const value of (Array.isArray(values) ? values : [values]).slice(0, 16)) add(value, "Entrypoint " + kind);
      }
      if (typeof json?.package === "string") {
        for (const kind of ["mixins", "client", "server"]) {
          if (!Array.isArray(json[kind])) continue;
          for (const name of json[kind].slice(0, 24)) if (typeof name === "string") add(json.package + "." + name, "Mixin " + kind);
        }
      }
    } catch { /* Descriptor data remains available in the raw evidence. */ }
  }
  return roots;
}

function behaviorTrace(entries, classes, resources, read, namespace) {
  const indexed = new Map(classes.map(entry => [entry.name.slice(0, -6), entry]));
  const hooks = declaredHooks(entries, read);
  const resourceIds = new Map();
  for (const entry of resources) {
    const found = /^(?:assets|data)\/([^/]+)\/[^/]+\/(.+)\.(?:json|png)$/i.exec(entry.name);
    if (!found) continue;
    const keys = new Set([found[2], found[2].replace(/^(?:block|item|entity)\//, "")]);
    for (const key of keys) {
      const id = found[1] + ":" + key;
      const paths = resourceIds.get(id) ?? [];
      if (paths.length < 4) paths.push(entry.name);
      resourceIds.set(id, paths);
    }
  }
  const graph = new Map(), missing = [];
  const loaderCandidates = classes.filter(entry => /\/(?:[\w$]*(?:Mod|Main|Init|Entrypoint|Registry))\.class$/.test(entry.name))
    .slice(0, 8);
  const seeds = new Set([...hooks.keys(), ...[...loaderCandidates, ...classes.slice(0, 12)]
    .map(entry => entry.name.slice(0, -6))]);
  const queue = [...seeds].map(owner => [owner, 0]);
  for (let cursor = 0; cursor < queue.length && graph.size < 64; cursor++) {
    const [owner, depth] = queue[cursor];
    graph.set(owner, null);
    const entry = indexed.get(owner);
    if (!entry) { if (hooks.has(owner)) missing.push(owner); continue; }
    try {
      const info = inspectBytecode(read(entry));
      if (!info) continue;
      graph.set(owner, info);
      if (depth >= 2) continue;
      const neighbors = new Set([
        ...info.methods.flatMap(row => row.edges.filter(edge => edge.kind !== "field").map(edge => edge.owner)),
        info.parent, ...info.implementsNames,
      ]);
      const next = [...neighbors].filter(name => indexed.has(name) && !seeds.has(name)).slice(0, 12);
      for (const name of next) { seeds.add(name); queue.push([name, depth + 1]); }
    } catch { /* A malformed class cannot invalidate the archive. */ }
  }
  const methods = [];
  for (const [owner, info] of graph) {
    if (!info) continue;
    for (const row of info.methods) {
      const data = [...new Set(row.strings.flatMap(value => resourceIds.get(
        value.includes(":") ? value : namespace ? namespace + ":" + value : "") ?? []))].slice(0, 4);
      const callback = hooks.has(owner) || info.loaderMarker || info.implementsNames.some(name => /Initializer|Event/.test(name));
      const edges = row.edges.filter(edge => edge.kind === "lambda target" || indexed.has(edge.owner)
        || hook.test(edge.owner + "." + edge.member)).slice(0, 6);
      const roles = [
        [/tick|update|scheduled/i, "simulation"], [/registr|initialize/i, "registration"],
        [/recipe|ingredient|inventory|extract|insert/i, "processing"], [/network|packet|payload/i, "network"],
        [/serializ|compoundtag|save|readnbt|writenbt/i, "persistence"], [/chunk|worldgen|blockpos/i, "world"],
        [/render|model|sprite|particle/i, "presentation"],
      ].filter(([pattern]) => pattern.test(row.method)
        || row.edges.some(edge => pattern.test(edge.owner) || pattern.test(edge.member))).map(([, role]) => role);
      const score = Number(callback) * 4 + Number(hook.test(row.method)) * 5 + data.length * 3
        + Number(row.allocations > 0) + Number(row.backEdges > 0)
        + Number(edges.some(edge => edge.kind === "lambda target")) * 3;
      if (score && (edges.length || data.length || row.allocations || row.backEdges)) {
        methods.push({ owner, method: row.method, descriptor: String(row.descriptor ?? "").slice(0, 160),
          roles, allocations: row.allocations, backEdges: row.backEdges, score, data,
          edges: edges.map(edge => (edge.kind + " @" + edge.pc + " " + edge.owner + "." + edge.member).slice(0, 240)) });
      }
    }
  }
  methods.sort((a, b) => b.score - a.score);
  const byRole = new Map();
  for (const method of methods) for (const role of method.roles) if (!byRole.has(role)) byRole.set(role, method);
  const sample = [...new Set([...byRole.values(), ...methods])].slice(0, 14);
  return { inspected: [...graph.values()].filter(Boolean).length, missing, declared: [...hooks],
    methods: sample.map(({ score, ...row }) => row) };
}

function bundledDependencies(bytes, entries) {
  return entries.filter(entry => /\.jar$/i.test(entry.name)).slice(0, 4).map(entry => {
    try {
      const archive = readEntry(bytes, entry, maxBundledBytes);
      if (!archive) return { path: entry.name, issue: "Unsupported archive or exceeds 4 MiB" };
      const members = directory(archive);
      const { declared } = inventory(members, member => readEntry(archive, member));
      return { path: entry.name, declared, classes: members.filter(member => member.name.endsWith(".class")).length };
    } catch {
      return { path: entry.name, issue: "Malformed nested archive" };
    }
  });
}

export async function inspectClass(bytes, query, entries = directory(bytes)) {
  if (query.includes("!")) {
    const [archivePath, classPath, extra] = query.split("!");
    if (extra !== undefined || !classPath?.trim()) throw new Error("Use archive.jar!class/path for one bundled JAR");
    const entry = entries.find(row => row.name === archivePath.trim() && /\.jar$/i.test(row.name));
    const archive = entry && readEntry(bytes, entry, maxBundledBytes);
    if (!archive) throw new Error("Bundled archive missing, unsupported, or larger than 4 MiB");
    const result = await inspectClass(archive, classPath);
    return { report: (entry.name + "!\n" + result.report).slice(0, 11400) };
  }
  const name = query.trim().replace(/\.class$/i, "").replaceAll(".", "/");
  if (!name) throw new Error("Enter a class path from the JAR evidence");
  const matches = entries.filter(entry =>
    entry.name === name + ".class" || entry.name.endsWith("/" + name + ".class"));
  if (matches.length !== 1) throw new Error(matches.length ? "Use the complete path to distinguish matching classes" : "Class not found in JAR");
  const entry = matches[0];
  const content = readEntry(bytes, entry);
  if (!content || content.length < 10 || content.readUInt32BE(0) !== 0xcafebabe) throw new Error("Class is invalid or exceeds the inspection limit");
  let report;
  let folder;
  try {
    folder = await mkdtemp(path.join(tmpdir(), "axiom-class-"));
    const file = path.join(folder, "Inspect.class");
    await writeFile(file, content, { flag: "wx" });
    const { stdout } = await promisify(execFile)(process.env.AXIOM_JAVAP_PATH || "javap",
      ["-c", "-p", "-s", file], { cwd: folder, timeout: 5000, maxBuffer: 256 * 1024, windowsHide: true });
    report = "JVM disassembly via javap\n" + stdout;
  } catch {
    report = "Built-in JVM instruction references. Set AXIOM_JAVAP_PATH to a JDK's javap for full disassembly.\n"
      + classLinks(content, true).join("\n");
  } finally {
    if (folder) {
      await rm(path.join(folder, "Inspect.class"), { force: true });
      await rmdir(folder);
    }
  }
  return { report: entry.name + "\n" + display(report).slice(0, 11000) };
}

export function inspectTexture(bytes, name, entries = directory(bytes)) {
  const entry = entries.find(item => item.name === name && /^assets\/[^/]+\/textures\/.+\.png$/i.test(name));
  if (!entry) throw new Error("Choose a PNG path from the JAR textures");
  const png = readEntry(bytes, entry);
  if (!png) throw new Error("Texture exceeds the inspection limit or uses unsupported compression");
  const dimensions = pngDimensions(new DataView(png.buffer, png.byteOffset, png.byteLength));
  if (dimensions.width > 512 || dimensions.height > 512) throw new Error("Texture study accepts PNGs up to 512 × 512");
  return { ...dimensions, png: png.toString("base64") };
}

export function inspectModel(bytes, name, size, entries = directory(bytes)) {
  const indexed = new Map(entries.filter(entry => modelEntry.test(entry.name)).map(entry => [entry.name, entry]));
  const read = path => {
    const entry = indexed.get(path);
    if (!entry) return;
    const data = readEntry(bytes, entry);
    return data ? JSON.parse(data.toString("utf8")) : undefined;
  };
  const choices = name ? [name] : [...indexed.keys()].sort((a, b) =>
    Number(/\/(?:block|geo)\//.test(b)) - Number(/\/(?:block|geo)\//.test(a)) || a.localeCompare(b)).slice(0, 20);
  for (const path of choices) {
    try {
      const model = read(path);
      if (!model) throw new Error("Model JSON was not found or exceeds the inspection limit");
      return { path, ...voxelPrototype(model, path, read, size) };
    } catch (error) {
      if (name) throw error;
    }
  }
  throw new Error("No supported cuboid model in the sampled JAR entries");
}

function recipeLeads(entries, read, limit = 64) {
  const recipes = entries.filter(entry => /^(?:assets|data)\/[^/]+\/recipes?\/.+\.json$/i.test(entry.name));
  const rows = [];
  for (const entry of recipes.slice(0, limit)) {
    try {
      const data = JSON.parse(read(entry)?.toString("utf8") ?? "null");
      const result = data?.result;
      const output = typeof result === "string" ? result : result?.id ?? result?.item;
      const type = data?.type;
      if (typeof output !== "string" || typeof type !== "string") continue;
      const ingredients = data.pattern && data.key
        ? data.pattern.flatMap(row => [...row].filter(c => c !== " ").map(c => data.key[c]))
        : data.ingredients ?? [data.ingredient];
      if (!Array.isArray(ingredients) || !ingredients.length || ingredients.length > 32) continue;
      const inputs = ingredients.map(item => typeof item === "string" ? item : item?.item ?? (item?.tag && "#" + item.tag));
      if (inputs.some(item => typeof item !== "string" || item.length > 120)) continue;
      const count = result?.count ?? 1;
      if (!Number.isSafeInteger(count) || count < 1) continue;
      const pattern = data.pattern ? data.pattern.map(row => [...row].map(c => {
        if (c === " ") return null;
        const item = data.key[c];
        return typeof item === "string" ? item : item?.item ?? "#" + item?.tag;
      })) : undefined;
      rows.push({ path: entry.name, type: type.slice(0, 120), output: output.slice(0, 120), count, inputs: inputs.sort(), pattern });
    } catch { /* Custom, malformed, and scripted recipes need manual evidence. */ }
  }
  return { total: recipes.length, rows };
}

/** Inspect a loose modpack ZIP in memory. No file from the archive is executed or extracted. */
export function inspectModArchive(bytes, { includePaths = false } = {}) {
  const entries = directory(bytes, maxPackBytes);
  const jars = entries.filter(entry => /\.jar$/i.test(entry.name));
  if (!jars.length || jars.length > 120) throw new Error("Expected between 1 and 120 JAR entries");
  let totalBytes = 0;
  const mods = jars.map(entry => {
    const name = entry.name;
    totalBytes += entry.size;
    try {
      if (totalBytes > maxPackExpandedBytes) throw new Error("Pack exceeds the total expanded JAR limit");
      const jar = readEntry(bytes, entry, maxJarBytes);
      if (!jar) throw new Error("Unreadable or oversized JAR");
      const artifacts = directory(jar);
      const paths = artifacts.map(artifact => artifact.name).sort();
      let readBudget = 48 * 1024 * 1024;
      let skippedReads = 0;
      const read = (artifact, limit) => {
        if (artifact.size > readBudget) { skippedReads++; return; }
        try {
          const content = readEntry(jar, artifact, limit);
          if (content) readBudget -= content.length;
          else skippedReads++;
          return content;
        } catch { skippedReads++; return undefined; }
      };
      const mods = inventory(artifacts, read);
      const classes = paths.filter(file => /\.class$/i.test(file));
      const resources = paths.filter(file => /^(?:assets|data)\//i.test(file));
      const recipes = recipeLeads(artifacts, read, Infinity);
      const languageFiles = artifacts.filter(artifact => /\/lang\/en_us\.(?:json|lang)$/i.test(artifact.name));
      const language = languageFiles.flatMap(artifact => {
        const value = read(artifact, 2 * 1024 * 1024)?.toString("utf8").replace(/^\uFEFF/, "") ?? "";
        try {
          return artifact.name.endsWith(".json") ? Object.keys(JSON.parse(value))
            : value.split(/\r?\n/).filter(line => /^[\w.:-]+=/.test(line)).map(line => line.split("=", 1)[0]);
        } catch { return []; }
      });
      const namespaces = [...new Set(resources.map(file => file.split("/")[1]))].sort();
      const leads = paths.filter(file => /(?:recipe|registration|registry|advancement|manual|patchouli|worldgen|loot_tables)/i.test(file)
        && !/\$|^META-INF\/maven/i.test(file));
      const firstClasses = artifacts.filter(artifact => artifact.name.endsWith(".class"))
        .sort((a, b) => Number(hook.test(b.name)) - Number(hook.test(a.name)));
      const trace = behaviorTrace(artifacts, firstClasses,
        artifacts.filter(artifact => /^(?:assets|data)\//i.test(artifact.name)), read, namespaces[0]);
      return { name, sha256: createHash("sha256").update(jar).digest("hex"), bytes: jar.length,
        files: paths.length, classes: classes.length, resources: resources.length, namespaces,
        declared: mods.declared, boundaries: mods.boundaries, bundled: bundledDependencies(jar, artifacts),
        recipeFiles: recipes.total, comparableRecipes: recipes.rows.length, englishKeys: language.length, skippedReads,
        trace: { classes: trace.inspected, entryPoints: trace.declared.map(([owner]) => owner).slice(0, 6),
          methods: trace.methods.slice(0, 5).map(({ owner, method, roles }) => ({ owner, method, roles })) },
        ...(includePaths && { paths, language, recipes: recipes.rows }),
        leads: leads.slice(0, 12) };
    } catch (error) {
      return { name, error: error.message };
    }
  });
  return { sha256: createHash("sha256").update(bytes).digest("hex"), entries: entries.length,
    otherFiles: entries.filter(entry => !/\.jar$/i.test(entry.name)).map(entry => entry.name), mods };
}

export const maxPackBytes = 160 * 1024 * 1024;
const maxPackExpandedBytes = 512 * 1024 * 1024;

function inventory(entries, extract) {
  const byName = new Map(entries.map(entry => [entry.name.toLowerCase(), entry]));
  const descriptors = [
    ["fabric.mod.json", "Fabric"], ["quilt.mod.json", "Quilt"], ["meta-inf/neoforge.mods.toml", "NeoForge"],
    ["meta-inf/mods.toml", "Forge-compatible"], ["mcmod.info", "Legacy Forge"],
  ];
  const declared = [];
  const licenseHints = [];
  for (const [file, loader] of descriptors) {
    const entry = byName.get(file);
    if (!entry) continue;
    const text = extract(entry)?.toString("utf8");
    let modIds = [], minecraft, dependencies = [];
    try {
      if (!text) throw new Error("Unreadable descriptor");
      // A declaration is evidence to review, never a grant to copy assets or code.
      const license = loader === "Fabric" ? JSON.parse(text).license
        : loader === "Quilt" ? JSON.parse(text).quilt_loader?.metadata?.license
        : /^\s*license\s*=\s*["']([^"'\r\n]*)["']/m.exec(text)?.[1];
      if (license) licenseHints.push({ path: entry.name, declaration: display(String(license)).slice(0, 240) });
      if (loader === "Fabric") {
        const mod = JSON.parse(text);
        modIds = [mod.id]; minecraft = mod.depends?.minecraft;
        dependencies = Object.keys(mod.depends ?? {});
      } else if (loader === "Quilt") {
        const mod = JSON.parse(text).quilt_loader;
        modIds = [mod?.id];
        const deps = Array.isArray(mod?.depends) ? mod.depends : [];
        dependencies = deps.map(dep => typeof dep === "string" ? dep : dep?.id);
        minecraft = deps.find(dep => dep?.id === "minecraft")?.versions;
      } else if (loader === "Legacy Forge") {
        const mod = JSON.parse(text);
        const rows = Array.isArray(mod) ? mod : mod.modList ?? [mod];
        modIds = rows.map(row => row?.modid);
        minecraft = rows.find(row => row?.mcversion)?.mcversion;
        dependencies = rows.flatMap(row => Array.isArray(row?.requiredMods) ? row.requiredMods : []);
      } else {
        const sections = text.split(/^\s*\[\[/m).slice(1);
        const value = (body, key) => new RegExp("^\\s*" + key + "\\s*=\\s*[\"']([^\"'\\r\\n]*)[\"']", "m").exec(body)?.[1];
        modIds = sections.filter(section => section.startsWith("mods]]")).map(section => value(section, "modId"));
        const deps = sections.filter(section => section.startsWith("dependencies."));
        dependencies = deps.map(section => value(section, "modId"));
        minecraft = value(deps.find(section => value(section, "modId") === "minecraft") ?? "", "versionRange");
      }
    } catch { /* A descriptor's presence alone identifies an intake lead. */ }
    const clean = values => [...new Set(values.filter(value => typeof value === "string").map(value => display(value).slice(0, 120)))].slice(0, 24);
    declared.push({ loader, file: entry.name, modIds: clean(modIds),
      minecraft: clean(Array.isArray(minecraft) ? minecraft : [minecraft]),
      dependencies: clean(dependencies).filter(id => id !== "minecraft") });
  }
  const boundaries = [];
  const has = pattern => entries.some(entry => pattern.test(entry.name));
  if (has(/mixins?\.json$|accesswidener$|(?:^|\/)accesstransformer\.cfg$/i)) boundaries.push("Engine patches or access changes");
  if (has(/\.jar$/i)) boundaries.push("Nested JARs require separate inspection");
  if (has(/\.(dll|so|dylib)$/i)) boundaries.push("Native library");
  if (!declared.length) boundaries.push("Loader or version declaration unavailable");
  return { declared, boundaries, licenseHints,
    licenseFiles: entries.filter(entry => /(?:^|\/)(?:licen[sc]e|copying|notice)(?:[._-]|$)/i.test(entry.name))
      .slice(0, 12).map(entry => entry.name) };
}

const cache = new Map();
const cacheLimit = 96 * 1024 * 1024;
let cachedBytes = 0;

export function cachedMod(digest) {
  const entry = cache.get(digest);
  if (!entry) throw new Error("JAR evidence expired. Import the JAR again.");
  cache.delete(digest);
  cache.set(digest, entry);
  return entry;
}

export function inspectMod(bytes, query = "") {
  const digest = createHash("sha256").update(bytes).digest("hex");
  let entries;
  if (cache.has(digest)) ({ entries } = cachedMod(digest));
  else {
    entries = directory(bytes);
    while (cachedBytes + bytes.length > cacheLimit || cache.size >= 6) {
      const oldest = cache.keys().next().value;
      cachedBytes -= cache.get(oldest).bytes.length;
      cache.delete(oldest);
    }
    cache.set(digest, { bytes, entries });
    cachedBytes += bytes.length;
  }
  const terms = [...new Set(query.toLowerCase().match(/[a-z0-9_]{3,}/g) ?? [])].slice(0, 24);
  const rank = name => {
    const lower = name.toLowerCase();
    return terms.reduce((score, term) => score + (lower.includes(term) ? 4 : 0), Number(hook.test(name)));
  };
  const ranked = list => list.map(entry => ({ entry, score: rank(entry.name) }))
    .sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name))
    .map(row => row.entry);
  const classes = entries.filter(entry => entry.name.endsWith(".class"));
  const resources = ranked(entries.filter(entry => /^(?:data|assets)\//.test(entry.name)));
  const info = entries.filter(entry => metadata.test(entry.name) || /mixins?\.json$/i.test(entry.name));
  const lines = [`SHA-256: ${digest}`, `${entries.length} files · ${classes.length} classes · ${resources.length} assets/data`,
    "Sampled static evidence. Method offsets identify field and call instructions, not executed paths. Constant-pool references may be unused. Behavior and mappings may need further inspection."];
  const extract = entry => {
    try { return readEntry(bytes, entry); } catch { return undefined; }
  };
  const coverage = inventory(entries, extract);
  coverage.bundled = bundledDependencies(bytes, entries);
  const recipes = recipeLeads(resources, extract);
  const ids = [...new Set(coverage.declared.flatMap(row => row.modIds))];
  const behavior = behaviorTrace(entries, ranked(classes), resources, extract, ids.length === 1 ? ids[0] : undefined);
  lines.push("Declared compatibility and dependency leads: " + JSON.stringify(coverage));
  lines.push("Behavior trace · " + behavior.inspected + " classes · static instruction counts, not runtime measurements");
  lines.push("Declared roots: " + behavior.declared.map(([owner, why]) => why + " " + owner).join("; ").slice(0, 1800));
  if (behavior.missing.length) lines.push("Unresolved declared classes: " + behavior.missing.join(", ").slice(0, 1200));
  const flow = behavior.methods.map(row => [
    row.owner + "." + row.method + row.descriptor + " [" + row.roles.join(", ") + "]",
    row.allocations + " allocation sites; " + row.backEdges + " backward branches",
    ...row.edges.map(edge => "  " + edge), ...row.data.map(file => "  resource ID candidate: " + file),
  ].join("\n")).join("\n\n");
  lines.push("Behavior leads:\n" + flow.slice(0, 6500));
  for (const entry of info.slice(0, 6)) {
    const content = extract(entry);
    if (content) lines.push(entry.name.slice(0, 180) + "\n" + display(content.toString("utf8")).slice(0, 650));
  }
  lines.push("Class paths:\n" + ranked(classes).slice(0, 14).map(entry => entry.name).join("\n").slice(0, 2400));
  const textures = resources.filter(entry => /^assets\/[^/]+\/textures\/.+\.png$/i.test(entry.name));
  const models = resources.filter(entry => modelEntry.test(entry.name));
  lines.push("Model paths:\n" + models.slice(0, 10).map(entry => entry.name.slice(0, 160)).join("\n"));
  lines.push("Texture paths:\n" + textures.slice(0, 12).map(entry => entry.name.slice(0, 160)).join("\n"));
  lines.push("Resource paths:\n" + resources.slice(0, 24).map(entry => entry.name.slice(0, 160)).join("\n"));
  for (const entry of resources.filter(entry => /^(?:data\/.*|assets\/[^/]+\/recipes\/.+)\.json$/i.test(entry.name)).slice(0, 4)) {
    const content = extract(entry);
    if (content) lines.push(entry.name.slice(0, 160) + "\n" + display(content.toString("utf8")).slice(0, 420));
  }
  return { report: lines.join("\n\n").slice(0, 18000), digest, coverage, recipes, behavior,
    features: [...behavior.methods.slice(0, 4).map(row => row.owner + "." + row.method + " → " + (row.edges[0] ?? row.data[0] ?? "static cost lead")),
      ...resources.filter(entry => /^data\//.test(entry.name)).slice(0, 8).map(entry => entry.name)]
      .map(entry => entry.slice(0, 300)),
    textures: textures.slice(0, 60).map(entry => entry.name),
    models: models.slice(0, 100).map(entry => entry.name) };
}
