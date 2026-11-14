import { catalogContract } from "./catalog-contracts.mjs";
import { foundryPerformanceContext, assertFoundryPerformanceFresh } from "./foundry-agent-context.mjs";
import { engineContext, assertEngineContextFresh } from "./engine-service.mjs";
import { createHash } from "node:crypto";
import { posix } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { files, read, root } from "./workspace.mjs";
import { canAssist, maxSources } from "./agent-contract.mjs";
import { boardTargets, focusLine } from "./context-targets.mjs";
import { cachedMod, inspectMod } from "./mod-evidence.mjs";
import { parityContext } from "./parity-data.mjs";

const exec = promisify(execFile);
const bundles = new Map();
const generic = new Set(["about", "after", "agent", "audit", "before", "change", "changes", "check", "checks", "could",
  "current", "edit", "edits", "find", "flaws", "from", "game", "improve", "implementation", "issue", "look", "make",
  "repair", "review", "should", "source", "that", "their", "this", "tool", "what", "where", "which", "with", "world"]);
const evidenceMarker = /\b(?:FIXME|TODO|HACK|workaround|unimplemented|panic|unsafe)\b/i;
const revision = text => createHash("sha256").update(text).digest("hex");
const terms = text => (text.match(/[A-Za-z][A-Za-z0-9_]{3,}/g) ?? []);
const unique = rows => [...new Set(rows)];

async function references(symbols, roots = ["crates", "assets", "docs", "tools/axiom-dev"]) {
  if (!symbols.length) return { hits: [], available: false };
  const parseHits = stdout => {
    const seen = new Set();
    return stdout.split("\n").flatMap(line => {
      const match = /^(.*?):(\d+):\d+:/.exec(line);
      if (!match) return [];
      const path = match[1].replaceAll("\\", "/"), number = Number(match[2]);
      const key = path + ":" + number;
      if (seen.has(key)) return [];
      seen.add(key);
      return [{ path, line: number }];
    });
  };
  try {
    const { stdout } = await exec("rg", ["--vimgrep", "--only-matching", "--color", "never", "--max-count", "3", "--max-columns", "300",
      "--max-filesize", "512K", "-e", "\\b(?:" + symbols.join("|") + ")\\b", "--", ...roots],
    { cwd: root, timeout: 2500, maxBuffer: 384_000, windowsHide: true });
    return { hits: parseHits(stdout), available: true };
  } catch (error) {
    return { hits: parseHits(error.stdout ?? ""), available: error.code === 1,
      reason: error.code === 1 ? undefined : error.code === "ERR_CHILD_PROCESS_STDIO_MAXBUFFER"
        ? "output limit reached" : error.killed ? "time limit reached" : String(error.code ?? "search failed") };
  }
}

async function contentTargets(owned, indexed, warnings) {
  const assets = owned.filter(file => /^assets\/.*\.(?:ron|json)$/.test(file));
  if (!assets.length) return [];
  const found = await references(assets.map(file => file.replaceAll(".", "\\.")), ["crates"]);
  if (!found.available) warnings.push("Content reference search incomplete; inspect native readers before editing");
  const result = [], counts = new Map(), documents = new Map();
  for (const hit of found.hits) {
    if (!indexed.has(hit.path) || !hit.path.endsWith(".rs")) continue;
    try {
      if (!documents.has(hit.path)) documents.set(hit.path, read(hit.path).text.split(/\r?\n/));
      const line = documents.get(hit.path)[hit.line - 1] ?? "";
      if (/^\s*(?:\/\/|\/\*|\*)/.test(line)) continue;
      const quoted = [...line.matchAll(/"([^"\r\n]*)"/g)].map(match => match[1].replaceAll("\\", "/"));
      for (const asset of assets) {
        if ((counts.get(asset) ?? 0) >= 2 || !quoted.some(value => value === asset
            || posix.normalize(posix.join(posix.dirname(hit.path), value)) === asset)) continue;
        result.push({ path: hit.path, line: hit.line, asset, kind: "Content path reference", readOnly: true });
        counts.set(asset, (counts.get(asset) ?? 0) + 1);
      }
    } catch { warnings.push("Content reference unavailable: " + hit.path); }
  }
  const registryPath = "tools/axiom-dev/catalog-contracts.mjs";
  try {
    if (indexed.has(registryPath)) {
      const registry = read(registryPath).text.split(/\r?\n/);
      for (const asset of assets) {
        const contract = catalogContract(asset);
        if (!contract) continue;
        const line = registry.findIndex(line => line.includes('id: "' + contract.id + '"')) + 1;
        if (line) result.push({ path: registryPath, line, asset, kind: "Catalog ownership contract", readOnly: true });
      }
    }
  } catch { warnings.push("Catalog ownership contract unavailable; inspect native readers before editing"); }
  return result;
}

function compilerDiagnostics(logs, indexed) {
  const rows = [];
  const short = /^((?:crates|assets|tools)\/[^:\s]+):(\d+)(?::\d+)?:\s*((?:error|warning)\b.*)$/i;
  const arrow = /^\s*-->\s*((?:crates|assets|tools)\/[^:\s]+):(\d+)(?::\d+)?/;
  let heading, expires = 0;
  for (const { message } of logs.slice(-120)) {
    if (typeof message !== "string") continue;
    const text = message.replaceAll("\\", "/").trimEnd();
    const direct = short.exec(text.trimStart());
    const location = direct ?? (expires > 0 && arrow.exec(text));
    if (location && indexed.has(location[1])) {
      rows.push({ path: location[1], line: Number(location[2]),
        message: (location[1] + ":" + location[2] + " · " + (direct ? direct[3] : heading)).slice(0, 420) });
      expires = 0;
    } else if (/^(?:error(?:\[E\d+\])?|warning): /i.test(text)) {
      heading = text;
      expires = 3;
    } else expires--;
  }
  return rows;
}

export async function collectContext(request, logs = []) {
  const { task, paths = [], symbols = [], targets = [], modDigests = [] } = request ?? {};
  if (typeof task !== "string" || !task.trim() || task.length > 2400 || !Array.isArray(paths) ||
      paths.length > maxSources || paths.some(path => !canAssist(path)) || !Array.isArray(symbols) ||
      !Array.isArray(targets) || targets.length > maxSources || targets.some(target =>
        !canAssist(target?.path) || (target.line !== undefined && (!Number.isSafeInteger(target.line) || target.line < 1)) ||
        (target.name !== undefined && (typeof target.name !== "string" || !/^[A-Za-z_]\w{0,119}$/.test(target.name))) ||
        (target.parts !== undefined && (!Array.isArray(target.parts) || target.parts.length > 16 ||
          target.parts.some(part => typeof part !== "string" || part.length > 240)))) ||
      symbols.length > 16 || symbols.some(word => typeof word !== "string" || !/^[A-Za-z][A-Za-z0-9_]{3,79}$/.test(word)) ||
      !Array.isArray(modDigests) || modDigests.length > 3 || modDigests.some(id => !/^[a-f0-9]{64}$/.test(id))) {
    throw new Error("Describe the issue and choose up to five project sources");
  }
  const modReferences = unique(modDigests).map(digest => {
    const { bytes } = cachedMod(digest);
    const inspected = inspectMod(bytes, task);
    return { digest, trust: "untrusted reference", reuse: "Original stand-ins only; license review required for reuse",
      coverage: inspected.coverage, recipes: inspected.recipes, behavior: inspected.behavior,
      report: inspected.report, textures: inspected.textures.slice(0, 12), models: inspected.models.slice(0, 10) };
  });
  const indexed = new Set(files(true).map(row => row.path).filter(canAssist));
  const owned = unique([...paths, ...targets.map(target => target.path)]).filter(path => indexed.has(path));
  if (owned.length > maxSources) throw new Error("Choose up to five project sources");
  const focus = new Map(), boardVersions = [], warnings = [];
  function add(target, via) {
    if (!indexed.has(target.path)) { warnings.push("Source unavailable · " + target.path); return; }
    const list = focus.get(target.path) ?? [];
    list.push({ ...target, via });
    focus.set(target.path, list);
  }
  targets.forEach(({ path, line, name, parts }) => add({ path, line, name, parts }));
  for (const path of owned.filter(path => path.startsWith("docs/axiom-notes/"))) {
    try {
      const doc = read(path);
      const pins = boardTargets(doc.text);
      boardVersions.push([path, doc.version]);
      pins.filter(pin => pin.path !== path).forEach(pin => add(pin, path));
    } catch { warnings.push("Could not read note · " + path); }
  }
  for (const target of await contentTargets(owned, indexed, warnings)) add(target);
  const names = unique([...symbols, ...targets.map(target => target.name).filter(Boolean),
    ...terms(task).filter(word => /_|[a-z][A-Z]/.test(word))]).slice(0, 16);
  const words = unique(terms(task.toLowerCase()).filter(word => !generic.has(word))).slice(0, 12);
  // Plain-language requests need content retrieval too, not just filename matches.
  const toolingScope = owned.length > 0 && owned.every(path => path.startsWith("tools/"));
  const found = await references(names.length ? names : words, toolingScope ? ["tools/axiom-dev", "docs"] : undefined);
  if (!found.available) warnings.push("Cross-reference search incomplete: " + (found.reason ?? "no search terms"));
  const matching = new Map();
  for (const hit of found.hits) {
    if (!indexed.has(hit.path)) continue;
    const rows = matching.get(hit.path) ?? [];
    rows.push(hit.line);
    matching.set(hit.path, rows);
  }
  const diagnostic = compilerDiagnostics(logs, indexed);
  const diagnosticsByPath = new Map();
  for (const row of diagnostic) {
    if (!diagnosticsByPath.has(row.path)) diagnosticsByPath.set(row.path, []);
    diagnosticsByPath.get(row.path).push(row);
  }
  const taskText = task.toLowerCase().replaceAll("\\", "/");
  const rank = path => {
    const lower = path.toLowerCase();
    if (toolingScope && /^(?:crates|assets)\//.test(path) && !focus.has(path) && !taskText.includes(lower)) return 0;
    if (!owned.some(source => source.startsWith("tools/")) && path.startsWith("tools/")
        && !focus.has(path) && !taskText.includes(lower) && !/\b(?:tooling|workbench|developer|forge)\b/i.test(task)) return 0;
    const matchingDiagnostic = diagnosticsByPath.get(path)?.some(row => words.some(word => row.message.toLowerCase().includes(word)));
    return Number(owned.includes(path)) * 18 + Number(focus.has(path)) * 16 + Math.min(3, matching.get(path)?.length ?? 0) * 7
      + Number(taskText.includes(lower)) * 24 + Number(Boolean(matchingDiagnostic)) * 8
      + words.reduce((score, word) => score + (lower.includes(word) ? 3 : 0), 0)
      + (owned.some(source => path.startsWith(source.slice(0, source.lastIndexOf("/") + 1))) ? 1 : 0);
  };
  const pinned = path => Number(owned.includes(path) || focus.has(path));
  const ranked = [...indexed].map(path => ({ path, score: rank(path) })).filter(row => row.score > 0)
    .sort((a, b) => pinned(b.path) - pinned(a.path) || b.score - a.score || a.path.localeCompare(b.path));
  const rows = [];
  let sampledFiles = 0;
  for (const { path, score } of ranked.slice(0, 26)) {
    try {
      const { text, version } = read(path);
      if (!text || text.length > 600_000) continue;
      sampledFiles++;
      const lines = text.split(/\r?\n/);
      const referenceLines = new Set(matching.get(path));
      const diagnosticLines = new Set(diagnosticsByPath.get(path)?.map(row => row.line));
      const focusLines = new Map();
      for (const target of focus.get(path) ?? []) {
        try {
          const line = focusLine({ path, text, kind: path.split(".").at(-1) }, target);
          if (!line || line > lines.length) throw new Error("Target moved");
          if (!focusLines.has(line)) focusLines.set(line, target);
        } catch { warnings.push("Saved target moved or is ambiguous · " + path); }
      }
      const inlineTests = lines.findIndex(line => /^\s*mod\s+tests\s*\{/.test(line));
      const testFile = /(?:_tests\.rs$|\/tests(?:\.rs|\/)|\.(?:test|spec)\.mjs$)/.test(path);
      const isDesign = path.startsWith("docs/");
      const candidates = [];
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i], lower = line.toLowerCase();
        const named = names.some(name => line.includes(name));
        const terms = words.reduce((n, word) => n + Number(lower.includes(word)), 0);
        const reference = referenceLines.has(i + 1);
        const target = focusLines.get(i + 1);
        const diagnosticHit = diagnosticLines.has(i + 1);
        const marker = evidenceMarker.test(line);
        const first = i < 5 && owned.includes(path);
        const test = testFile || (inlineTests >= 0 && i > inlineTests);
        if (!target && !diagnosticHit && !named && !terms && !reference && !first && !(marker && score >= 10)) continue;
        if (test && !target && !diagnosticHit && !named && !reference && terms < 2) continue;
        if (isDesign && !target && !named && !reference && !owned.includes(path) && !taskText.includes(path.toLowerCase())
            && terms < 2) continue;
        const declaration = /\b(?:fn|struct|enum|trait|type|const)\s+([A-Za-z0-9_]+)/.exec(line)?.[1];
        const owner = declaration && names.includes(declaration);
        candidates.push({ i, test, via: target?.via, asset: target?.asset,
          repairEligible: !test && !target?.readOnly && Boolean(target?.line || target?.name || target?.parts?.length ||
            diagnosticHit || owner || reference || named || terms >= 2),
          score: Number(Boolean(target)) * 30 + Number(diagnosticHit) * 14 + (named ? 8 : 0) + (reference ? 10 : 0) + terms * 3
          + Number(marker) * 2 + Number(first) + Number(Boolean(owner)) * 5,
        kind: target ? target.kind ?? (target.via ? "System board pin" : "Attached source") : diagnosticHit ? "Diagnostic location"
          : owner ? "Possible owner" : named || (reference && names.length) ? "Symbol reference" : reference ? "Lexical reference" : marker ? "Risk marker" : "Related source" });
      }
      candidates.sort((a, b) => b.score - a.score);
      const picked = [];
      const bestTest = candidates.find(hit => hit.test);
      const order = bestTest && !testFile ? [...candidates.filter(hit => !hit.test).slice(0, 8), bestTest] : candidates;
      for (const hit of order) {
        if (picked.length >= 2 && !hit.test) continue;
        if (!hit.test && !focusLines.has(hit.i + 1) && picked.some(line => Math.abs(line - hit.i) < 15)) continue;
        const first = Math.max(0, hit.i - 6);
        const end = Math.min(lines.length, hit.i + 9);
        const excerpt = lines.slice(first, end).map(line => line.slice(0, 250)).join("\n").slice(0, 1800);
        if (!excerpt.trim()) continue;
        const kind = hit.test ? "Existing test source"
          : isDesign ? "Design or review note" : hit.kind;
        rows.push({ path, line: hit.i + 1, first: first + 1, last: end, excerpt, version, kind,
          via: hit.via, asset: hit.asset, repairEligible: hit.repairEligible,
          score: score + hit.score });
        picked.push(hit.i);
        if (picked.length >= (bestTest && !testFile ? 3 : 2)) break;
      }
    } catch { /* A removed or unreadable file does not invalidate other evidence. */ }
  }
  rows.sort((a, b) => b.score - a.score || a.path.localeCompare(b.path));
  const tests = rows.find(row => row.kind === "Existing test source");
  const design = rows.find(row => row.kind === "Design or review note");
  const focused = rows.filter(row => row.kind === "Attached source" || row.kind === "System board pin").slice(0, 6);
  const attached = owned.map(path => rows.find(row => row.path === path));
  const consumers = owned.map(asset => rows.find(row => row.asset === asset));
  const seenPaths = new Set();
  const diverse = rows.filter(row => {
    if (seenPaths.has(row.path)) return false;
    seenPaths.add(row.path);
    return true;
  }).slice(0, 6);
  const evidence = [...new Set([...focused, ...attached, tests, design, ...consumers, ...diverse, ...rows])]
    .filter(Boolean).slice(0, 12).map(({ score, ...row }) => row);
  const related = new Set([...owned, ...evidence.map(row => row.path)]);
  const runtime = request.runtime === true ? await engineContext(task) : undefined;
  const packet = { task, evidence, modReferences, runtime, performance: foundryPerformanceContext(task), parity: parityContext(task), diagnostics: diagnostic.filter(row => related.has(row.path)).slice(-8),
    symbols: names, crossReferences: found.available, sampledFiles,
    warnings: unique(warnings).slice(0, 8), boardVersions };
  const id = revision(JSON.stringify(packet));
  if (bundles.size >= 16) bundles.delete(bundles.keys().next().value);
  bundles.set(id, packet);
  return { id, ...packet };
}

export function assertContextFresh(packet) {
  assertEngineContextFresh(packet.runtime);
  assertFoundryPerformanceFresh(packet.performance);
  for (const reference of packet.modReferences ?? []) {
    if (revision(cachedMod(reference.digest).bytes) !== reference.digest) {
      throw new Error("Reference archive changed. Import it and gather context again.");
    }
  }
  const versions = new Map([...packet.boardVersions, ...packet.evidence.map(row => [row.path, row.version])]);
  const changed = [...versions].some(([path, version]) => {
    try { return read(path).version !== version; } catch { return true; }
  });
  if (changed) throw new Error("Context changed on disk. Gather it again before running.");
}

export function loadContext(id, task) {
  if (id === undefined) return undefined;
  const packet = bundles.get(id);
  if (!packet || packet.task !== task) throw new Error("Gather context again for this task");
  assertContextFresh(packet);
  return packet;
}
