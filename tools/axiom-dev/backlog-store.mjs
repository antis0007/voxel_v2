import { randomUUID } from "node:crypto";
import { closeSync, openSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { ensureDirectory, state } from "./workspace.mjs";

const file = path.join(state, "backlog.json");
const lock = file + ".lock";
const priorities = ["Urgent", "High", "Normal", "Low"];

function load() {
  try { return JSON.parse(readFileSync(file, "utf8")); }
  catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

function conflict(message) {
  throw Object.assign(new Error(message), { status: 409 });
}

function required(value, label, max) {
  if (typeof value !== "string" || !value.trim() || value.trim().length > max) {
    throw new Error(`${label} must be between 1 and ${max} characters.`);
  }
  return value.trim();
}

function priority(value) {
  if (!priorities.includes(value)) throw new Error("Choose a valid priority.");
  return value;
}

function sourcePaths(value = "") {
  if (typeof value !== "string" || value.length > 3000) throw new Error("Source paths must fit within 3000 characters.");
  const paths = [...new Set(value.split(/\r?\n/).map(line => line.trim()).filter(Boolean))];
  if (paths.length > 12 || paths.some(file => file.includes("\\") || file.includes(":")
    || file.split("/").some(part => !part || part === "." || part === ".."))) {
    throw new Error("Enter up to 12 repository-relative paths, one per line, with forward slashes.");
  }
  return paths;
}

function find(rows, id) {
  const task = rows.find(row => row.id === id);
  if (!task) throw Object.assign(new Error("Task not found."), { status: 404 });
  return task;
}

export function readBacklog(id) {
  const rows = load();
  if (id) return find(rows, id);
  return rows.map(({ history, ...task }) => ({
    ...task,
    verification: task.verification?.result,
  }));
}

export function changeBacklog(input) {
  if (!input || typeof input !== "object") throw new Error("Expected a backlog command.");
  const actor = required(input.actor, "Actor", 100);
  ensureDirectory(state);
  let fd;
  try { fd = openSync(lock, "wx"); }
  catch (error) {
    if (error.code === "EEXIST") conflict("Backlog is busy. Refresh before retrying.");
    throw error;
  }
  const temporary = file + "." + randomUUID() + ".tmp";
  try {
    const rows = load(), now = new Date().toISOString();
    let task;
    if (input.action === "create") {
      task = {
        id: randomUUID(), revision: 0, status: "Ready", owner: "", history: [],
        title: required(input.title, "Title", 160),
        acceptance: required(input.acceptance, "Acceptance criteria", 4000),
        priority: priority(input.priority ?? "Normal"),
        paths: sourcePaths(input.paths),
      };
      rows.push(task);
    } else {
      task = find(rows, input.id);
      if (task.revision !== input.revision) conflict("Task changed. Review the latest state before retrying.");
      transition(task, input, actor);
    }
    task.revision++;
    task.updatedAt = now;
    task.history.push({
      action: input.action, actor, at: now, status: task.status, owner: task.owner,
      ...historyDetails(task, input.action),
      ...(input.action === "reopen" ? { note: input.note.trim() } : {}),
    });
    writeFileSync(temporary, JSON.stringify(rows), { flag: "wx" });
    renameSync(temporary, file);
    return task;
  } finally {
    try { unlinkSync(temporary); }
    catch (error) { if (error.code !== "ENOENT") throw error; }
    finally { closeSync(fd); unlinkSync(lock); }
  }
}

function transition(task, input, actor) {
  if (task.status === "Done" && input.action !== "reopen") conflict("Reopen the task to change it.");
  if (input.action === "reopen" && task.status !== "Done") conflict("Only completed tasks can be reopened.");
  if (["block", "release", "handoff"].includes(input.action) && task.owner !== actor) {
    conflict("This action requires the current owner.");
  }
  switch (input.action) {
    case "edit": {
      const acceptance = required(input.acceptance, "Acceptance criteria", 4000);
      const paths = input.paths === undefined ? task.paths : sourcePaths(input.paths);
      if (acceptance !== task.acceptance || JSON.stringify(paths) !== JSON.stringify(task.paths)) delete task.verification;
      task.title = required(input.title, "Title", 160);
      task.acceptance = acceptance;
      task.priority = priority(input.priority);
      task.paths = paths;
      break;
    }
    case "claim":
      if (task.owner || !["Ready", "Review"].includes(task.status)) conflict("This task is unavailable to claim.");
      task.owner = actor;
      if (task.status === "Ready") task.status = "In progress";
      break;
    case "assign":
      task.owner = required(input.owner, "New owner", 100);
      if (task.status === "Ready") task.status = "In progress";
      break;
    case "release":
      task.owner = "";
      if (task.status === "In progress") task.status = "Ready";
      break;
    case "block":
      task.blocker = required(input.note, "Blocker", 4000);
      task.status = "Blocked";
      delete task.verification;
      break;
    case "unblock":
      if (task.status !== "Blocked") conflict("This task has no active blocker.");
      delete task.blocker;
      task.status = task.owner ? "In progress" : "Ready";
      break;
    case "handoff":
      if (!["In progress", "Review"].includes(task.status)) conflict("Resolve the blocker before handing off.");
      task.handoff = {
        ref: required(input.ref, "Change reference", 500),
        note: required(input.note, "Handoff summary", 4000),
        revision: task.revision + 1,
      };
      delete task.verification;
      task.status = "Review";
      task.owner = "";
      break;
    case "verify":
      if (task.status !== "Review") conflict("Submit a handoff before recording verification.");
      if (!["Pass", "Fail"].includes(input.result)) throw new Error("Choose Pass or Fail.");
      task.verification = {
        result: input.result,
        note: required(input.note, "Verification evidence", 4000),
        handoffRevision: task.handoff.revision,
      };
      break;
    case "complete":
      if (task.status !== "Review" || task.verification?.result !== "Pass"
        || task.verification.handoffRevision !== task.handoff.revision) {
        conflict("Completion requires a recorded pass for the current handoff.");
      }
      task.status = "Done";
      task.owner = "";
      break;
    case "reopen":
      required(input.note, "Reopen reason", 4000);
      task.status = "Ready";
      delete task.handoff;
      delete task.verification;
      break;
    default: throw new Error("Unknown backlog action.");
  }
}

function historyDetails(task, action) {
  switch (action) {
    case "create":
    case "edit": return { title: task.title, acceptance: task.acceptance, priority: task.priority, paths: task.paths };
    case "block": return { note: task.blocker };
    case "handoff": return task.handoff;
    case "verify": return task.verification;
    default: return {};
  }
}
