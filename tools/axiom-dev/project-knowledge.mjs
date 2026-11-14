import * as fs from "node:fs";
import * as path from "node:path";
import { root, ensureDirectory, files, read, revision } from "./workspace.mjs";
import { parse } from "./syntax.mjs";
import { noteRoot, isNote, isSystemBoard, systemBoardMarker, slug, noteLinks, targetRows, rustTunables, displayKey } from "./targets.mjs";

const catalog = entry => ["ron", "json"].includes(entry.kind)
  && /^assets\//.test(entry.path) && !/^assets\/(source|concepts|art_direction)\//.test(entry.path);
const cache = new Map();
let sourceIndex;
const normalized = value => value.toLowerCase().replace(/[_-]/g, " ");
const fileLabel = path => path.split("/").at(-1);
const noteTitle = (path, text) => /^#\s+(.+)$/m.exec(text)?.[1].trim()
  || fileLabel(path).replace(/\.md$/, "").replace(/-/g, " ");

function catalogRows(entry) {
  const stamp = revision(entry.path);
  let known = cache.get(entry.path);
  if (known?.stamp !== stamp) {
    let rows = [];
    try {
      const doc = read(entry.path);
      rows = targetRows(parse(doc.text, doc.kind), entry.path);
    } catch { /* The file can still be found and opened as Source. */ }
    known = { stamp, rows };
    cache.set(entry.path, known);
  }
  return known.rows;
}

export function searchProject(query, fieldsOnly) {
  const words = normalized(String(query ?? "").slice(0, 160)).trim().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const phrase = words.join(" ");
  const hits = [];
  function offer(row) {
    if (fieldsOnly && !["field", "rust"].includes(row.kind)) return;
    const label = normalized(row.label);
    const description = normalized([row.context, row.path, row.value].filter(Boolean).join(" "));
    if (!words.every(word => label.includes(word) || description.includes(word))) return;
    const score = words.reduce((total, word) => total + (label === word ? 12 : label.startsWith(word) ? 8
      : label.includes(word) ? 5 : 1), 0) + (label === phrase ? 16 : 0)
      + (row.kind === "note" ? 3 : row.kind === "record" ? 2 : 0);
    hits.push({ row, score });
  }
  for (const entry of files()) {
    if (!fieldsOnly) {
      if (isNote(entry.path)) {
        try {
          const { text } = read(entry.path);
          offer({ path: entry.path, label: noteTitle(entry.path, text), kind: "note",
            context: text.slice(0, 1024).replace(/\s+/g, " ") });
        } catch { /* A moved note is omitted. */ }
      } else {
        offer({ path: entry.path, label: fileLabel(entry.path), kind: "file" });
      }
    }
    if (catalog(entry)) {
      try { for (const row of catalogRows(entry)) offer(row); }
      catch { cache.delete(entry.path); }
    }
  }
  if (!sourceIndex) {
    sourceIndex = [];
    for (const entry of files()) {
      if (!/^crates\/.*\.rs$/.test(entry.path) || /(?:_tests|\/tests)\.rs$|\/tests\//.test(entry.path)) continue;
      try {
        for (const row of rustTunables(read(entry.path).text)) {
          sourceIndex.push({ path: entry.path, kind: "rust", name: row.name, type: row.type, value: row.raw,
            label: displayKey(row.name.toLowerCase()), context: fileLabel(entry.path) });
        }
      } catch { /* The source file remains searchable by name. */ }
    }
  }
  for (const row of sourceIndex) offer(row);
  return hits.sort((a, b) => b.score - a.score || a.row.label.localeCompare(b.row.label))
    .slice(0, 36).map(hit => hit.row);
}

export function invalidateKnowledge() {
  sourceIndex = undefined;
}

export function notesIndex() {
  const notes = [];
  for (const entry of files().filter(entry => isNote(entry.path)).slice(0, 200)) {
    try {
      const doc = read(entry.path);
      notes.push({ path: entry.path, title: noteTitle(entry.path, doc.text), links: noteLinks(doc.text),
        systemBoard: isSystemBoard(doc.text) });
    } catch { /* A moved note is omitted. */ }
  }
  return notes;
}

export function createNote(title, systemBoard = false, purpose = "") {
  const clean = typeof title === "string" ? title.trim().replace(/[\r\n]+/g, " ").slice(0, 120) : "";
  const stem = slug(clean);
  if (!stem || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/.test(stem)) throw new Error("Choose another note title");
  const relative = noteRoot + stem + ".md";
  const directory = ensureDirectory(path.join(root, noteRoot.slice(0, -1)));
  const brief = typeof purpose === "string" ? purpose.trim().slice(0, 500) : "";
  const text = "# " + clean + "\n\n" + (systemBoard
    ? systemBoardMarker + "\n\n" + (brief ? "## Purpose\n\n" + brief + "\n\n" : "") : "");
  try { fs.writeFileSync(path.join(directory, stem + ".md"), text, { flag: "wx" }); }
  catch (error) {
    if (error.code === "EEXIST") throw new Error("A note with this title already exists");
    throw error;
  }
  files(true);
  return { path: relative };
}

