import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { parse } from "./syntax.mjs";
import { isSheet, readSheet } from "./sprite-sheet.mjs";
import { recipePath, zeroRecipeIngredient } from "./idea-data.mjs";
import { prepareProposal, verificationSource } from "./agent-contract.mjs";
import { maxText, read } from "./workspace.mjs";
import { assertFrozenRustTests } from "./verification-guard.mjs";

const digest = text => createHash("sha256").update(text).digest("hex");

function check(file, text) {
  const ext = path.extname(file).toLowerCase();
  if (Buffer.byteLength(text) > maxText) throw new Error("Candidate exceeds the editor's size limit");
  if (isSheet(file)) {
    readSheet(text, file);
    return { name: "Sprite sheet structure", status: "Passed" };
  }
  if (ext === ".json" || ext === ".ron") {
    const parsed = parse(text, ext.slice(1));
    if (file === recipePath) {
      const invalid = zeroRecipeIngredient(parsed);
      if (invalid) throw new Error(invalid.name + " · Ingredient " + (invalid.index + 1) + " must have a positive count.");
    }
    return { name: ext === ".json" ? "JSON syntax" : "RON structure", status: "Passed" };
  }
  if (ext === ".mjs") {
    const result = spawnSync(process.execPath, ["--input-type=module", "--check"], {
      input: text, encoding: "utf8", timeout: 5000, maxBuffer: 64_000, windowsHide: true,
      env: { ...process.env, NODE_OPTIONS: "" },
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(result.stderr.split(/\r?\n/).find(line => line.startsWith("SyntaxError:")) || "JavaScript syntax check failed");
    }
    return { name: "JavaScript syntax", status: "Passed" };
  }
  return { name: "Native or manual validation", status: "Pending" };
}

// The verifier works from saved versions independently of the model's output
// and does not execute candidate code. Draft-only excerpts stay reviewable.
export function verifyCandidate(proposal, sources, { protectTests = false } = {}) {
  const changed = new Set(proposal.edits.map(edit => edit.source));
  const pins = sources.filter(source => changed.has(source.id));
  if (!pins.length) return { status: "No changes", files: [] };
  const live = new Map();
  const pending = [];
  const draftPaths = new Set(pins.filter(source => !source.anchor).map(source => source.path));
  for (const source of pins) {
    if (draftPaths.has(source.path)) {
      if (!pending.some(row => row.path === source.path)) pending.push({ path: source.path, name: "Unsaved baseline", status: "Pending" });
      continue;
    }
    if (!live.has(source.path)) live.set(source.path, read(source.path));
    const doc = live.get(source.path);
    if (doc.version !== source.anchor.version || typeof doc.text !== "string" ||
        doc.text.slice(source.anchor.start, source.anchor.start + source.text.length) !== source.text) {
      return { status: "Stale", files: [{ path: source.path, name: "Saved baseline", status: "Failed", detail: "Saved source changed or the excerpt no longer matches." }] };
    }
  }
  const edits = proposal.edits.filter(edit => !draftPaths.has(sources.find(source => source.id === edit.source).path));
  const verified = sources.filter(source => live.has(source.path) && changed.has(source.id)).map(source => {
    const doc = live.get(source.path);
    return { ...source, doc, original: doc.text, start: source.anchor.start };
  });
  const files = [...pending];
  for (const { doc, text } of prepareProposal({ ...proposal, edits }, verified)) {
    const file = [...live].find(([, value]) => value === doc)[0];
    const fingerprint = { before: doc.version, after: digest(text) };
    try {
      if (protectTests) {
        if (verificationSource(file)) throw new Error("Automatic improvement cannot edit verification sources");
        if (file.endsWith(".rs")) assertFrozenRustTests(doc.text, text);
      }
      files.push({ path: file, ...check(file, text), ...fingerprint });
    }
    catch (error) { files.push({ path: file, name: "Candidate validation", status: "Failed", detail: error.message.slice(0, 420), ...fingerprint }); }
  }
  return {
    status: files.some(row => row.status === "Failed") ? "Failed"
      : files.some(row => row.status === "Pending") ? "Checks pending" : "Syntax checked",
    files,
  };
}
