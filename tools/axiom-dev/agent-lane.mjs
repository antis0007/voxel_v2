import { randomUUID } from "node:crypto";
import { closeSync, openSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { ensureDirectory, state } from "./workspace.mjs";

export function createAgentLane(filename = "agent-lane.lock") {
  const file = path.join(state, filename);
  let held;

  function agentLane() {
    let owner;
    try { owner = JSON.parse(readFileSync(file, "utf8")); }
    catch (error) { if (error.code === "ENOENT") return undefined; throw error; }
    if (!Number.isInteger(owner.pid) || owner.pid <= 0 || typeof owner.id !== "string") throw new Error("Invalid AI lane record. Retain it for inspection.");
    let alive = true;
    try { process.kill(owner.pid, 0); }
    catch (error) { if (error.code === "ESRCH") alive = false; }
    return { ...owner, alive };
  }

  function acquireAgentLane(label) {
    ensureDirectory(state);
    let fd;
    try { fd = openSync(file, "wx"); }
    catch (error) {
      if (error.code === "EEXIST") throw new Error("The AI lane is occupied. Wait for the active run, or recover an interrupted mission.");
      throw error;
    }
    const owner = { id: randomUUID(), pid: process.pid, label, at: new Date().toISOString() };
    try { writeFileSync(fd, JSON.stringify(owner)); }
    catch (error) { closeSync(fd); unlinkSync(file); throw error; }
    closeSync(fd);
    held = owner.id;
    return owner.id;
  }

  function assertAgentLane(id) {
    if (!id || held !== id || agentLane()?.id !== id) throw new Error("AI lane ownership was lost; stop and inspect the mission.");
  }

  function releaseAgentLane(id) {
    assertAgentLane(id);
    unlinkSync(file);
    held = undefined;
  }

  // Explicit recovery only. A live (including reused) PID is never displaced.
  function recoverAgentLane(expectedId) {
    const owner = agentLane();
    if (!owner) return;
    if (expectedId && owner.id !== expectedId) throw new Error("AI lane changed; retry recovery.");
    if (owner.alive) throw new Error("The recorded AI worker is still alive. Stop it before recovery.");
    if (agentLane()?.id !== owner.id) throw new Error("AI lane changed; refresh before recovery.");
    unlinkSync(file);
  }

  return { agentLane, acquireAgentLane, assertAgentLane, releaseAgentLane, recoverAgentLane };
}

export const { agentLane, acquireAgentLane, assertAgentLane, releaseAgentLane, recoverAgentLane } = createAgentLane();
