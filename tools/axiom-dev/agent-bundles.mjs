import { randomUUID } from "node:crypto";
import { existsSync, lstatSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { canAssist } from "./agent-contract.mjs";
import { ensureDirectory, read, restoreTextBundle, saveTextBundle, state } from "./workspace.mjs";

const folder = path.join(state, "candidate-bundles");
const uuid = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/;
const filename = /^\d{13}-[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}\.json$/;
const hash = /^[a-f0-9]{64}$/;
const backup = /^\d{13}-[a-f0-9]{12}$/;

function manifest(id) {
  if (typeof id !== "string" || !filename.test(id + ".json")) throw new Error("Invalid bundle ID");
  const file = path.join(ensureDirectory(folder), id + ".json");
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.size > 8192) throw new Error("Invalid bundle record");
  const record = JSON.parse(readFileSync(file, "utf8"));
  if ((record.revertPolicy !== undefined && record.revertPolicy !== "retain-food-identity")
      || record.id !== id || !Array.isArray(record.files) || !record.files.length || record.files.length > 5
      || new Set(record.files.map(row => row?.path)).size !== record.files.length
      || record.files.some(row => !canAssist(row?.path) || !hash.test(row.before)
        || !hash.test(row.after) || !backup.test(row.backupId) || row.before === row.after)) {
    throw new Error("Invalid bundle record");
  }
  return record;
}

export function publishBundle(changes, runId, { preserveIdentity = false } = {}) {
  if (typeof preserveIdentity !== "boolean") throw new Error("Invalid bundle identity policy");
  const at = Date.now();
  const id = at + "-" + randomUUID();
  let published;
  const saved = saveTextBundle(changes, files => {
    const directory = ensureDirectory(folder);
    const target = path.join(directory, id + ".json");
    const temporary = target + ".tmp";
    try {
      writeFileSync(temporary, JSON.stringify({ id, at, runId: uuid.test(runId) ? runId : undefined,
        revertPolicy: preserveIdentity ? "retain-food-identity" : undefined, files }), { flag: "wx" });
      renameSync(temporary, target);
      published = id;
    } finally {
      rmSync(temporary, { force: true });
    }
  });
  return { id: published, saved: saved.map((doc, index) => ({ ...doc, path: changes[index].path })) };
}

export function recentBundles() {
  if (!existsSync(folder)) return [];
  return readdirSync(ensureDirectory(folder)).filter(name => filename.test(name))
    .sort().reverse().slice(0, 20).map(name => manifest(name.slice(0, -5)));
}

export function decisionBundles() {
  if (!existsSync(folder)) return [];
  const names = readdirSync(ensureDirectory(folder)).filter(name => filename.test(name));
  if (names.length > 1000) throw new Error("Candidate history needs explicit archival before more automatic planning");
  return names.sort().map(name => manifest(name.slice(0, -5)));
}

export function inspectBundle(id) {
  const record = manifest(id);
  const files = record.files.map(file => {
    let version;
    try { version = read(file.path).version; } catch { /* A missing or unavailable file is a conflict. */ }
    return { ...file, status: version === file.before ? "Original" : version === file.after ? "Applied" : "Changed" };
  });
  return { ...record, files, reversible: record.revertPolicy !== "retain-food-identity" && files.some(file => file.status === "Applied")
    && files.every(file => file.status !== "Changed") };
}

export function revertBundle(id) {
  const record = inspectBundle(id);
  if (record.revertPolicy === "retain-food-identity") throw new Error("Food identity is retained permanently. Use the food workflow to withdraw its recipe.");
  if (!record.reversible) throw new Error("Bundle has changed on disk or is already at its original versions.");
  const files = record.files.filter(file => file.status === "Applied");
  return restoreTextBundle(files).map((doc, index) => ({ ...doc, path: files[index].path }));
}
