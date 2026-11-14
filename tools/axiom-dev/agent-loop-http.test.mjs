import { after, test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import { once } from "node:events";

const directory = mkdtempSync(path.join(tmpdir(), "forge-loop-test-"));
after(() => rmSync(directory, { recursive: true, force: true }));
const tools = path.join(directory, "tools", "axiom-dev");
mkdirSync(tools, { recursive: true });
for (const entry of readdirSync(fileURLToPath(new URL(".", import.meta.url)))) {
  if (entry.endsWith(".mjs")) copyFileSync(new URL(entry, import.meta.url), path.join(tools, entry));
}
const load = name => import(pathToFileURL(path.join(tools, name)).href);
const put = (file, text) => {
  mkdirSync(path.dirname(path.join(directory, file)), { recursive: true });
  writeFileSync(path.join(directory, file), text);
};
const owner = "crates/app/src/neutral.rs";
const caller = "crates/app/src/integration.rs";
const original = "// Shipping capacity is checked before inventory consumption.\n"
  + "pub fn transfer_inventory(stock: u32, capacity: u32) -> bool {\n    stock < capacity\n}\n\n"
  + "#[cfg(test)]\nmod tests {\n    #[test]\n    fn inventory_capacity() { assert!(!super::transfer_inventory(4, 4)); }\n}\n";
put(owner, original);
put(caller, "// Shipping inventory capacity caller\nfn dispatch() { transfer_inventory(0, 4); }\n");
put("docs/design/inventory.md", "# Shipping inventory capacity\nA design proposal, not executed verification.\n");
put("crates/app/src/new_owner.rs", "fn freshness_probe() { /* shipping inventory capacity */ }\n");
const request = { task: "Inspect shipping inventory capacity before consumption", paths: [owner], lane: "engine" };

test("authenticated HTTP routes propose and deduplicate; failed provider never succeeds", async () => {
  const child = spawn(process.execPath, [path.join(tools, "server.mjs")], { windowsHide: true,
    env: { ...process.env, AXIOM_CODEX_PATH: path.join(directory, "missing-provider.exe") }, stdio: ["ignore", "pipe", "pipe"] });
  let output = "", errors = "";
  child.stderr.on("data", data => { errors += data; });
  const address = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Server timeout: " + errors)), 15000);
    child.stdout.on("data", data => {
      output += data;
      const match = /http:\/\/127\.0\.0\.1:\d+\/#([a-f0-9]+)/.exec(output);
      if (match) { clearTimeout(timeout); resolve(match[0]); }
    });
    child.once("error", reject);
    child.once("exit", () => { clearTimeout(timeout); reject(new Error(errors)); });
  });
  try {
    const url = new URL(address), base = url.origin;
    assert.equal((await fetch(base + "/api/improvement/decisions")).status, 403);
    const call = async (route, body) => {
      const response = await fetch(base + route, { method: body ? "POST" : "GET",
        headers: { "x-axiom-session": url.hash.slice(1), "content-type": "application/json", origin: base },
        body: body ? JSON.stringify(body) : undefined });
      const data = await response.json();
      assert(response.ok, JSON.stringify(data));
      return data;
    };
    const staleMetadata = await fetch(base + "/api/improvement/next", { method: "POST",
      headers: { "x-axiom-session": url.hash.slice(1), "x-axiom-fabrication-schema": "stale", "content-type": "application/json", origin: base },
      body: JSON.stringify({ ...request, paths: [caller] }) });
    assert.equal(staleMetadata.status, 409);
    assert.match((await staleMetadata.json()).error, /Reload Forge/);
    assert.equal((await fetch(base + "/fabrication-contract.mjs")).status, 200);
    assert.equal((await fetch(base + "/workshop-planning.mjs")).status, 200);
    const result = await call("/api/improvement/next", { ...request, paths: [caller] });
    assert.equal(result.decision.path, caller);
    await call("/api/improvement", { provider: "codex", model: "", tasks: [{ decisionId: result.decision.id }] });
    let queue;
    for (let attempt = 0; attempt < 100; attempt++) {
      queue = await call("/api/improvement");
      if (queue.phase !== "Running") break;
      await new Promise(resolve => setTimeout(resolve, 30));
    }
    assert.equal(queue.phase, "Complete");
    assert.equal(queue.rows[0].status, "Failed");
    const record = (await call("/api/improvement/decisions")).find(row => row.id === result.decision.id);
    assert.equal(record.status, "blocked");
    assert.equal(record.verification, "pending");
    assert.equal((await call("/api/improvement/next", { ...request, paths: [caller] })).decision, null);
    await call("/api/improvement/loop", { ...request, paths: [caller], provider: "codex", model: "", maxSteps: 2 });
    let loop;
    for (let attempt = 0; attempt < 100; attempt++) {
      loop = await call("/api/improvement");
      if (loop.phase !== "Running") break;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    assert.equal(loop.phase, "Needs evidence");
    assert.equal(loop.rows.length, 0, "unchanged failed attempt must not invoke the provider again");
    const freshPath = "crates/app/src/new_owner.rs";
    await call("/api/improvement/loop", { ...request, paths: [freshPath], provider: "codex", model: "", maxSteps: 1 });
    for (let attempt = 0; attempt < 100; attempt++) {
      loop = await call("/api/improvement");
      if (loop.phase !== "Running") break;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    assert.equal(loop.phase, "Blocked");
    assert.equal(loop.rows.length, 1);
    assert.equal(loop.rows[0].status, "Failed");
  } finally {
    child.kill();
    await once(child, "exit");
  }
});
