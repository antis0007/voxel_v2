import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomBytes } from "node:crypto";
import { parse } from "./syntax.mjs";
import { validateNativeContent, validateNativeBundle, validateStableCatalogIds } from "./native-content-validation.mjs";
import { fabricationPath, fabricationRows } from "./fabrication-data.mjs";
import { artSource, isSheet, isPng, readSheet } from "./sprite-sheet.mjs";
import { pngDimensions } from "./pixel-grid.mjs";
import { parseVoxel } from "./voxel-model.mjs";

export const root = fs.realpathSync(fileURLToPath(new URL("../../", import.meta.url)));
export const state = path.join(root, "target", "axiom-dev");
export const maxText = 4 * 1024 * 1024;
export const maxAsset = 24 * 1024 * 1024;
const navigationFile = path.join(state, "navigation.json");
const workbenchFile = path.join(state, "workbench.json");
const skip = new Set(["target", "artifacts", "downloads", "saves", "vendor", "MC-RS", "node_modules", "__pycache__"]);
const sources = new Set([".rs", ".wgsl", ".toml", ".md", ".txt", ".ps1", ".sh", ".py", ".js", ".mjs",
  ".ts", ".css", ".html", ".yml", ".yaml", ".csv", ".tsv", ".lock", ".gpl", ".ron", ".json", ".svg", ".glsl", ".glbmeta"]);
const pictures = { ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
  ".webp": "image/webp", ".gif": "image/gif", ".ico": "image/x-icon" };
const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const signature = bytes => createHash("sha256").update(bytes).digest("hex");
let index;

export function inside(base, file) {
  const rel = path.relative(base, file);
  return rel && rel !== ".." && !rel.startsWith(".." + path.sep) && !path.isAbsolute(rel);
}

export function ensureDirectory(directory) {
  if (!inside(root, directory)) throw new Error("Directory is outside the project");
  const parent = path.dirname(directory);
  if (parent !== root) ensureDirectory(parent);
  if (!fs.existsSync(directory)) fs.mkdirSync(directory);
  if (fs.realpathSync(directory) !== directory || !fs.statSync(directory).isDirectory()) {
    throw new Error("Project directory redirects elsewhere");
  }
  return directory;
}

function format(relative) {
  const ext = path.extname(relative).toLowerCase();
  if (ext === ".vxplan" || /^assets\/models\/factory\/[^/]+\.vox$/.test(relative)) return "text";
  if (ext === ".ron" || ext === ".json") return ext.slice(1);
  if (sources.has(ext) || ["justfile", ".gitignore", ".gitattributes"].includes(relative)) return "text";
  return relative.startsWith("assets/") ? "asset" : null;
}

function walk(relative, found) {
  for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
    const name = relative ? relative + "/" + entry.name : entry.name;
    if (entry.isSymbolicLink() || skip.has(name) || skip.has(entry.name)
      || (entry.name.startsWith(".") && ![".cargo", ".gitignore", ".gitattributes"].includes(entry.name))
      || name.endsWith(".log")) continue;
    if (entry.isDirectory()) walk(name, found);
    if (entry.isFile()) {
      const kind = format(name);
      if (kind) found.set(name, { path: name, kind, sprite: spriteInfo(name) });
    }
  }
}

function spriteInfo(relative) {
  let art = artSource(relative);
  if (!art) return art;
  if (art.generator && !fs.existsSync(path.join(root, art.generator))) {
    const { generator, ...rest } = art;
    art = { ...rest, issue: "Registered generator is unavailable in this checkout" };
  }
  if (isSheet(relative)) {
    try { readSheet(readBytes(path.join(root, relative), maxText).toString("utf8"), relative); return art; }
    catch (error) { return { ...art, mode: "reference", issue: error.message }; }
  }
  if (!isPng(relative)) return art;
  try {
    const bytes = readBytes(path.join(root, relative), maxAsset);
    return { ...art, ...pngDimensions(new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)), revision: signature(bytes) };
  } catch (error) { return { ...art, mode: art.mode === "source" ? "reference" : art.mode, issue: error.message }; }
}

export function files(refresh = false) {
  if (refresh || !index) {
    const found = new Map();
    walk("", found);
    index = new Map([...found].sort(([a], [b]) => a.localeCompare(b)));
  }
  return [...index.values()];
}

function existing(relative) {
  if (!index) files();
  const item = index.get(relative);
  if (!item) throw new Error("Select an indexed project file");
  const file = path.join(root, relative);
  if (fs.realpathSync(file) !== file || !fs.statSync(file).isFile()) throw new Error("File was moved or redirected");
  return { ...item, file };
}

export function revision(relative) {
  const { file } = existing(relative);
  const stat = fs.statSync(file);
  return stat.mtimeMs + ":" + stat.size;
}

function navigationEntries(entries) {
  if (!Array.isArray(entries)) return [];
  if (!index) files();
  const seen = new Set();
  const result = [];
  for (const entry of entries.slice(0, 8)) {
    if (!entry || !index.has(entry.path) || seen.has(entry.path)) continue;
    seen.add(entry.path);
    const trail = Array.isArray(entry.trail) ? entry.trail.slice(0, 96) : [];
    const sprite = {};
    for (const key of ["entry", "ramp", "tileWidth", "tileHeight", "column", "row"]) {
      const value = entry.sprite?.[key];
      if (Number.isInteger(value) && value >= 0 && value <= 2048) sprite[key] = value;
    }
    result.push({ path: entry.path,
      view: ["fields", "graph", "sprite", "asset", "source"].includes(entry.view) ? entry.view : undefined,
      trail: trail.every(index => Number.isInteger(index) && index >= 0 && index < 150000) ? trail : [],
      sprite });
  }
  return result;
}

export function readNavigation() {
  try {
    if (fs.realpathSync(navigationFile) !== navigationFile) return [];
    return navigationEntries(JSON.parse(readBytes(navigationFile, 16_384).toString("utf8")));
  } catch { return []; }
}

export function writeNavigation(entries) {
  const recent = navigationEntries(entries);
  writeSettings(navigationFile, recent);
  return recent;
}

function workbenchSettings(data) {
  const pins = Array.isArray(data?.pins) ? data.pins.slice(0, 20).filter(pin =>
    typeof pin?.section === "string" && pin.section.length <= 32 &&
    typeof pin?.key === "string" && pin.key.length <= 512 &&
    typeof pin?.label === "string" && pin.label.length <= 140)
    .map(({ section, key, label }) => ({ section, key, label })) : [];
  return { layout: data?.layout === "rows" ? "rows" : "tiles", density: data?.density === "roomy" ? "roomy" : "compact",
    font: data?.font === "plain" ? "plain" : "pixel", side: data?.side === "left" ? "left" : "right",
    width: Number.isFinite(data?.width) ? Math.max(30, Math.min(65, data.width)) : 42, pins };
}

export function readWorkbench() {
  try {
    if (fs.realpathSync(workbenchFile) === workbenchFile) return workbenchSettings(JSON.parse(readBytes(workbenchFile, 32768).toString("utf8")));
  } catch { /* First launch. */ }
  return workbenchSettings();
}

export function writeWorkbench(data) {
  const settings = workbenchSettings(data);
  writeSettings(workbenchFile, settings);
  return settings;
}

function writeSettings(file, data) {
  ensureDirectory(state);
  const temporary = file + "." + randomBytes(6).toString("hex");
  try {
    fs.writeFileSync(temporary, JSON.stringify(data), { flag: "wx" });
    fs.renameSync(temporary, file);
  } finally {
    if (fs.existsSync(temporary)) fs.unlinkSync(temporary);
  }
}

function readBytes(file, limit) {
  if (fs.statSync(file).size > limit) throw new Error("File exceeds the editor size limit");
  const bytes = fs.readFileSync(file);
  if (bytes.length > limit) throw new Error("File exceeds the editor size limit");
  return bytes;
}

export function read(relative) {
  const { file, kind } = existing(relative);
  const size = fs.statSync(file).size;
  const image = pictures[path.extname(file).toLowerCase()];
  if (kind === "asset" && size > maxAsset) return { kind, size, image: false };
  const bytes = readBytes(file, kind === "asset" ? maxAsset : maxText);
  return { kind, size, image: Boolean(image), version: signature(bytes),
    text: kind === "asset" ? undefined : utf8.decode(bytes), backups: backups(relative),
    recovery: isSheet(relative) ? readSpriteDraft(relative) : undefined };
}

const spriteDraftFile = relative => path.join(state, "sprite-" + signature(relative) + ".json");
function readSpriteDraft(relative) {
  const file = spriteDraftFile(relative);
  try {
    if (fs.realpathSync(file) === file) return JSON.parse(readBytes(file, 4 * maxText).toString("utf8"));
  } catch { /* There may be no recovery file. */ }
}

export function writeSpriteDraft(relative, { previous, version, original, text }) {
  if (!isSheet(relative)) throw new Error("Choose an authored sprite sheet");
  existing(relative);
  const active = readSpriteDraft(relative);
  if ((active?.id ?? null) !== (previous ?? null)) throw new Error("A different Forge session changed this sprite recovery copy.");
  const file = spriteDraftFile(relative);
  if (text == null) {
    if (active) fs.unlinkSync(file);
    return { id: null };
  }
  if (typeof text !== "string" || typeof original !== "string" || Buffer.byteLength(text) > maxText
    || Buffer.byteLength(original) > maxText || signature(original) !== version) throw new Error("Invalid sprite recovery data");
  const id = randomBytes(12).toString("hex");
  writeSettings(file, { id, version, original, text });
  return { id };
}

export function preview(relative) {
  const { file } = existing(relative);
  const mime = pictures[path.extname(file).toLowerCase()];
  if (!mime) throw new Error("No image preview for this format");
  return { mime, bytes: readBytes(file, maxAsset) };
}

export function modelBytes(relative) {
  const { file } = existing(relative);
  if (!relative.startsWith("assets/") || !/\.glb$/i.test(relative)) throw new Error("Choose a project GLB mesh");
  return readBytes(file, maxAsset);
}

function backupPath(relative) {
  return path.join(state, "backups", signature(relative));
}

function backups(relative) {
  const directory = backupPath(relative);
  if (!fs.existsSync(directory)) return [];
  if (fs.realpathSync(directory) !== directory) throw new Error("Backup directory redirects elsewhere");
  return fs.readdirSync(directory).filter(name => /^\d{13}-[a-f0-9]{12}$/.test(name)).sort().reverse();
}

export function history(relative, id) {
  const { kind } = existing(relative);
  if (!backups(relative).includes(id)) throw new Error("Backup not found");
  const file = path.join(backupPath(relative), id);
  if (fs.realpathSync(file) !== file || !fs.statSync(file).isFile()) throw new Error("Backup was redirected");
  return readBytes(file, kind === "asset" ? maxAsset : maxText);
}

function prepareSave(relative, { version, text, asset, restore }) {
  const { file, kind } = existing(relative);
  const limit = kind === "asset" ? maxAsset : maxText;
  const original = readBytes(file, limit);
  if (signature(original) !== version) throw new Error("Changed on disk. Copy the draft or reload before saving.");
  let next;
  if (restore) next = history(relative, restore);
  else if (kind === "asset" && typeof asset === "string") next = Buffer.from(asset, "base64");
  else if (kind !== "asset" && typeof text === "string") next = Buffer.from(text);
  else throw new Error("No replacement data");
  if (next.length > limit) throw new Error("Replacement exceeds the editor size limit");
  if (!restore && relative === fabricationPath) {
    const rows = fabricationRows(utf8.decode(next));
    if (rows.length < parse(utf8.decode(original), "ron").children.length) {
      throw new Error("Keep existing Fabricator recipes in place. Placed machines save their recipe IDs.");
    }
  } else if (!restore && (kind === "ron" || kind === "json")) parse(utf8.decode(next), kind);
  if (!restore && isSheet(relative)) readSheet(utf8.decode(next), relative);
  if (!restore && /^assets\/models\/factory\/[^/]+\.vox$/.test(relative)) parseVoxel(utf8.decode(next));
  if (kind === "ron") {
    validateStableCatalogIds(relative, utf8.decode(original), utf8.decode(next));
    validateNativeContent(root, relative, utf8.decode(next));
  }
  return { relative, file, version, limit, original, next };
}

function commitSaves(plans, beforePublish) {
  validateNativeBundle(root, plans);
  const staged = [], committed = [];
  try {
    for (const plan of plans) {
      const { relative, file, original, next } = plan;
      if (original.equals(next)) continue;
      const backup = ensureDirectory(backupPath(relative));
      const stamp = Date.now() + "-" + randomBytes(6).toString("hex");
      const temporary = path.join(path.dirname(file), ".axiom-" + stamp);
      const rollback = plans.length > 1 ? temporary + ".original" : undefined;
      const options = { flag: "wx", mode: fs.statSync(file).mode };
      staged.push({ ...plan, temporary, rollback, backupId: stamp });
      fs.writeFileSync(path.join(backup, stamp), original, { flag: "wx" });
      if (rollback) fs.writeFileSync(rollback, original, options);
      fs.writeFileSync(temporary, next, options);
    }
    if (staged.length) beforePublish?.(staged.map(({ relative, version, next, backupId }) =>
      ({ path: relative, before: version, after: signature(next), backupId })));
    for (const entry of staged) {
      const { relative, file, version, limit, temporary } = entry;
      if (existing(relative).file !== file || signature(readBytes(file, limit)) !== version) {
        throw new Error("Changed on disk during save: " + relative);
      }
      fs.renameSync(temporary, file);
      committed.push(entry);
    }
  } catch (error) {
    const conflicts = [];
    for (const entry of committed.reverse()) {
      try {
        if (existing(entry.relative).file !== entry.file ||
            signature(readBytes(entry.file, entry.limit)) !== signature(entry.next)) throw new Error();
        fs.renameSync(entry.rollback, entry.file);
      } catch { conflicts.push(entry.relative); }
    }
    if (conflicts.length) throw new Error(error.message + ". Restore these files from File info → Backups: " + conflicts.join(", "));
    throw error;
  } finally {
    for (const { temporary, rollback } of staged) {
      fs.rmSync(temporary, { force: true });
      if (rollback) fs.rmSync(rollback, { force: true });
    }
  }
  return plans.map(plan => read(plan.relative));
}

export function save(relative, change) {
  return commitSaves([prepareSave(relative, change)])[0];
}

export function saveTextBundle(changes, beforePublish) {
  return commitSaves(changes.map(({ path: relative, version, text }) => prepareSave(relative, { version, text })), beforePublish);
}

export function restoreTextBundle(files) {
  return commitSaves(files.map(({ path: relative, before, after, backupId }) => {
    const plan = prepareSave(relative, { version: after, restore: backupId });
    if (signature(plan.next) !== before) throw new Error("Backup checksum mismatch: " + relative);
    return plan;
  }));
}

export function usages(relative) {
  existing(relative);
  const filename = relative.split("/").at(-1);
  const entries = [];
  for (const { path: entry } of files()) {
    if ((!entry.startsWith("crates/") && !entry.startsWith("tools/")) || !/\.(rs|ps1|py)$/.test(entry)) continue;
    const file = existing(entry).file;
    if (fs.statSync(file).size > maxText) continue;
    const source = fs.readFileSync(file, "utf8");
    if (!source.includes(filename)) continue;
    for (const [i, line] of source.split("\n").entries()) {
      if (!line.includes(filename)) continue;
      const start = line.indexOf(filename);
      const prefix = line.slice(0, start);
      const quoted = /["']([^"']*)$/.exec(prefix)?.[1];
      if (!quoted?.includes("assets/") || !relative.startsWith("assets/")) continue;
      if (!quoted.slice(quoted.indexOf("assets/")).concat(filename).endsWith(relative)) continue;
      entries.push({ path: entry, line: i + 1, text: line.trim().slice(0, 220) });
      if (entries.length >= 24) return entries;
    }
  }
  return entries;
}

