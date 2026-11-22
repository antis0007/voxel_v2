import { randomUUID } from "node:crypto";
import { linkSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync, statSync } from "node:fs";
import path from "node:path";
import { ensureDirectory, state } from "./workspace.mjs";

// Epoch 2 queues are invisible to already-running legacy schedulers. Do not copy
// runnable rows between stores; legacy history remains authoritative for its IDs.
export const workflowStorageVersion = 2;
const file = path.join(state, "workflows-v2.json");
const lock = file + ".lock";
const legacyFile = path.join(state, "workflows.json");
const archiveDirectory = path.join(state, "workflow-archive");
export const workflowCapacity = 100;
const renameWait = new Int32Array(new SharedArrayBuffer(4));
let archivedIndex;
const readSnapshots = new Map();

// Windows readers/indexers can briefly deny replacement even while our writer
// lock is held. Never delete the destination or fall back to an in-place write.
function replaceHistory(temporary) {
  const deadline = Date.now() + 1200;
  for (;;) {
    try { renameSync(temporary, file); return; }
    catch (error) {
      if (!["EPERM", "EACCES", "EBUSY"].includes(error.code) || Date.now() >= deadline) throw error;
      Atomics.wait(renameWait, 0, 0, 40);
    }
  }
}
function readHistory(file, snapshot = false) {
  try {
    const stat = statSync(file, { bigint: true });
    if (stat.size > 32 * 1024 * 1024) throw new Error("Mission history exceeds 32 MB. Archive completed missions before continuing.");
    const revision = snapshot && [stat.ino, stat.size, stat.mtimeNs, stat.ctimeNs].join(":");
    if (snapshot && readSnapshots.get(file)?.revision === revision) return readSnapshots.get(file).rows;
    const rows = JSON.parse(readFileSync(file, "utf8"));
    if (!Array.isArray(rows) || rows.length > workflowCapacity) throw new Error("Invalid mission history; retain it for inspection.");
    if (snapshot) readSnapshots.set(file, { revision, rows });
    return rows;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    readSnapshots.delete(file);
    return [];
  }
}

// Status readers may share an immutable snapshot. Mutation always parses its
// own copy while holding the write lock.
export function loadWorkflows(snapshot = false) { return readHistory(file, snapshot); }
export function loadLegacyWorkflows(snapshot = false) { return readHistory(legacyFile, snapshot); }

// Called at startup while holding the exclusive Dev server lease.
export function recoverWorkflowStore() {
  let owner;
  try { owner = JSON.parse(readFileSync(lock, "utf8")); }
  catch (error) { if (error.code === "ENOENT") return; throw error; }
  if (!Number.isSafeInteger(owner.pid) || owner.pid <= 0) throw new Error("Invalid mission history lock owner.");
  try { process.kill(owner.pid, 0); }
  catch (error) {
    if (error.code !== "ESRCH") throw error;
    rmSync(lock);
  }
}

export function mutateWorkflows(change) {
  ensureDirectory(state);
  const id = randomUUID();
  const owner = lock + "." + id;
  try {
    writeFileSync(owner, JSON.stringify({ pid: process.pid }), { flag: "wx" });
    linkSync(owner, lock);
  }
  catch (error) {
    if (error.code === "EEXIST") throw Object.assign(new Error("Mission history is busy. Refresh before retrying."), { status: 409 });
    throw error;
  } finally {
    try { rmSync(owner, { force: true }); }
    catch (error) { process.stderr.write("Mission lock cleanup: " + error.message + "\n"); }
  }
  const temporary = file + "." + id + ".tmp";
  try {
    const rows = loadWorkflows();
    const result = change(rows);
    const json = JSON.stringify(rows);
    if (Buffer.byteLength(json) > 32 * 1024 * 1024) throw new Error("Mission history is full; archive completed missions.");
    writeFileSync(temporary, json, { flag: "wx" });
    replaceHistory(temporary);
    return result;
  } finally {
    try { rmSync(temporary, { force: true }); }
    finally { rmSync(lock); }
  }
}

export function workflowEvent(mission, kind, detail, step) {
  mission.revision++;
  mission.updatedAt = new Date().toISOString();
  mission.events.push({ at: mission.updatedAt, kind, detail, step });
}

function archiveRow(rows, index) {
  const mission = rows[index];
  const { id, revision } = mission;
  if (!/^[a-f0-9-]{36}$/.test(id)) throw new Error("Invalid mission ID.");
  const archive = path.join(ensureDirectory(archiveDirectory), id + "-" + revision + ".json");
  const json = JSON.stringify(mission, null, 2);
  const temporary = archive + "." + randomUUID() + ".tmp";
  try {
    writeFileSync(temporary, json, { flag: "wx" });
    linkSync(temporary, archive);
  }
  catch (error) {
    if (error.code !== "EEXIST" || readFileSync(archive, "utf8") !== json) throw error;
  } finally { rmSync(temporary, { force: true }); }
  rows.splice(index, 1);
  archivedIndex = undefined;
  return { archived: id, path: archive };
}

export function makeWorkflowRoom(rows, { followUpOf, reserve = () => false } = {}) {
  if (rows.length < workflowCapacity) return;
  const referenced = new Set(rows.map(row => row.followUpOf?.id));
  const eligible = row => row.id !== followUpOf && !referenced.has(row.id);
  let index = rows.findIndex(row => eligible(row) && ["Complete", "Cancelled"].includes(row.phase));
  if (index < 0) index = rows.findIndex(row => eligible(row) && ["Deferred", "Blocked"].includes(row.phase)
    && row.settings.publication === "automatic" && !row.waitingForSource && !row.retryAt && !row.stopping
    && !reserve(row));
  if (index < 0) throw new Error("All mission slots are reserved by live or scheduled work. New work can be admitted once a slot is released.");
  archiveRow(rows, index);
}

export function readArchivedWorkflow(id) {
  if (!/^[a-f0-9-]{36}$/.test(id)) return;
  let names;
  try { names = readdirSync(archiveDirectory); }
  catch (error) { if (error.code === "ENOENT") return; throw error; }
  const name = names.filter(name => name.startsWith(id + "-") && /^\d+\.json$/.test(name.slice(id.length + 1)))
    .sort((a, b) => Number.parseInt(b.slice(id.length + 1), 10) - Number.parseInt(a.slice(id.length + 1), 10))[0];
  if (name) return JSON.parse(readFileSync(path.join(archiveDirectory, name), "utf8"));
}

export function archivedWorkflows() {
  let modified;
  try { modified = statSync(archiveDirectory).mtimeMs; }
  catch (error) { if (error.code === "ENOENT") return []; throw error; }
  if (archivedIndex?.modified === modified) return archivedIndex.rows;
  const records = new Map();
  for (const name of readdirSync(archiveDirectory)) {
    const match = /^([a-f0-9-]{36})-(\d+)\.json$/.exec(name);
    if (!match) continue;
    const [, id, rawRevision] = match;
    const revision = Number(rawRevision);
    if ((records.get(id)?.revision ?? -1) < revision) records.set(id, { name, revision });
  }
  const rows = [...records.values()].flatMap(({ name }) => {
    try {
      const { id, parityId, objective, phase, updatedAt, autonomousRevision, error } = JSON.parse(readFileSync(path.join(archiveDirectory, name), "utf8"));
      return [{ id, parityId, objective, phase, updatedAt, autonomousRevision, error }];
    } catch (error) {
      process.stderr.write("Mission archive retained but unavailable: " + name + " · " + (error.code ?? error.name) + "\n");
      return [];
    }
  }).sort((a, b) => (b.updatedAt ?? "").localeCompare(a.updatedAt ?? ""));
  archivedIndex = { modified, rows };
  return rows;
}

export function archiveWorkflow(id, revision) {
  return mutateWorkflows(rows => {
    const index = rows.findIndex(row => row.id === id);
    const mission = rows[index];
    if (!mission || mission.revision !== revision) throw Object.assign(new Error("Mission changed. Refresh before archiving."), { status: 409 });
    if (!["Complete", "Cancelled"].includes(mission.phase)) throw new Error("Complete or cancel a mission before archiving it.");
    return archiveRow(rows, index);
  });
}
