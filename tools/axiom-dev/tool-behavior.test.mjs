import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn, spawnSync } from "node:child_process";

test("real native tool behaviors use one reversible, revision-checked command path", async t => {
  const root = fileURLToPath(new URL("../../", import.meta.url));
  const base = path.join(root, "target/tool-behavior-tests");
  fs.mkdirSync(base, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(base, "fixture-"));
  let server;
  try {
    for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]) fs.copyFileSync(path.join(root, file), path.join(fixture, file));
    for (const file of ["assets/materials.ron", "assets/tools/parts.ron", "tools/axiom-dev", "crates/core", "crates/content", "crates/audio", "crates/tools", "target/axiom-dev/native-validation"]) {
      fs.mkdirSync(path.dirname(path.join(fixture, file)), { recursive: true });
      fs.cpSync(path.join(root, file), path.join(fixture, file), { recursive: true });
    }
    const module = name => import(pathToFileURL(path.join(fixture, "tools/axiom-dev", name)));
    const { toolBehaviorCommand: command, toolBehaviorPath } = await module("tool-behavior.mjs");
    const { read, save } = await module("workspace.mjs");
    const original = read(toolBehaviorPath);
    const initial = command("inspect", { schemaId: "pickaxe" });
    const capabilities = { ...initial.schemas[0].capabilities, blocking: true,
      mining: { ...initial.schemas[0].capabilities.mining, charged: true } };
    const request = { schemaId: "pickaxe", version: initial.version, capabilities };
    await t.test("native discovery and real schema selection remain in sync", () => {
      assert.ok(initial.miningStyles.includes(initial.schemas[0].capabilities.mining.class));
      assert.deepEqual(initial.schemas[0].parts, ["head", "binding", "handle"]);
      assert.equal(initial.schemas[0].capabilities.mining.charged, false);
      assert.ok(command("inspect").schemas.some(row => row.id === "shield" && row.capabilities.blocking));
    });
    await t.test("invalid combinations and malformed requests cannot publish", () => {
      assert.throws(() => command("preview", { ...request, capabilities: { ...capabilities,
        mining: { ...capabilities.mining, fells_trees: true } } }), /tree felling requires Cut/);
      assert.throws(() => command("preview", { ...request, capabilities: { blocking: "yes", mining: null } }), /boolean/);
      assert.throws(() => command("preview", { ...request, capabilities: { ...capabilities, invented: true } }), /Unexpected/);
      assert.throws(() => command("preview", { ...request, schemaId: "invented" }), /existing/);
      assert.equal(read(toolBehaviorPath).version, original.version);
      assert.deepEqual(read(toolBehaviorPath).backups, original.backups);
    });
    await t.test("semantic no-ops ignore JSON property order", () => {
      const before = initial.schemas[0].capabilities;
      const result = command("preview", { ...request, capabilities: { mining: before.mining, blocking: before.blocking } });
      assert.equal(result.changed, false);
      assert.equal(result.approval, null);
      assert.throws(() => command("apply", { ...request, capabilities: before }), /Preview this exact/);
    });
    await t.test("preview is read-only and approval binds exact native-validated behavior", () => {
      const preview = command("preview", request);
      assert.equal(read(toolBehaviorPath).version, original.version);
      assert.equal(preview.files.length, 1);
      assert.equal(preview.after.blocking, true);
      assert.throws(() => command("apply", { ...request, capabilities: { ...capabilities, mining: null }, approval: preview.approval }), /exact change/);
    });
    await t.test("apply preserves every other schema and can undo exactly", () => {
      const before = command("inspect");
      const preview = command("preview", request);
      const applied = command("apply", { ...request, approval: preview.approval });
      const after = command("inspect");
      assert.deepEqual(after.schemas.filter(row => row.id !== "pickaxe"), before.schemas.filter(row => row.id !== "pickaxe"));
      assert.deepEqual(after.schemas.find(row => row.id === "pickaxe").capabilities, capabilities);
      assert.throws(() => command("apply", { ...request, approval: preview.approval }), /changed on disk/);
      command("revert", { bundle: applied.bundle });
      assert.equal(read(toolBehaviorPath).text, original.text);
    });
    await t.test("stale previews and conflicting undo preserve newer source", () => {
      const preview = command("preview", request);
      const changed = save(toolBehaviorPath, { version: original.version, text: original.text + "\n// external edit\n" });
      assert.throws(() => command("apply", { ...request, approval: preview.approval }), /changed on disk/);
      save(toolBehaviorPath, { version: changed.version, text: original.text });
      const applied = command("apply", { ...request, approval: preview.approval });
      const current = read(toolBehaviorPath);
      const newer = save(toolBehaviorPath, { version: current.version, text: current.text + "\n// newer edit\n" });
      assert.throws(() => command("revert", { bundle: applied.bundle }), /changed on disk/);
      assert.equal(read(toolBehaviorPath).version, newer.version);
      save(toolBehaviorPath, { version: newer.version, text: current.text });
      command("revert", { bundle: applied.bundle });
    });
    await t.test("the CLI and authenticated HTTP actions use the same native commands", async () => {
      const cli = spawnSync(process.execPath, [path.join(fixture, "tools/axiom-dev/tool-behavior-cli.mjs"), "preview"],
        { cwd: fixture, input: JSON.stringify(request), encoding: "utf8", windowsHide: true });
      assert.equal(cli.status, 0, cli.stderr);
      const expected = JSON.parse(cli.stdout);
      server = spawn(process.execPath, [path.join(fixture, "tools/axiom-dev/server.mjs")], { cwd: fixture, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
      const url = await new Promise((resolve, reject) => {
        let text = "", errors = "";
        server.stderr.on("data", data => { errors = (errors + data).slice(-4000); });
        const timeout = setTimeout(() => reject(new Error("Forge server startup timed out: " + errors)), 30000);
        server.on("error", error => { clearTimeout(timeout); reject(error); });
        server.on("exit", code => { clearTimeout(timeout); reject(new Error("Forge server exited " + code + ": " + errors)); });
        server.stdout.on("data", data => { text += data; const match = text.match(/http:\/\/127\.0\.0\.1:\d+\/#\w+/); if (match) { clearTimeout(timeout); resolve(new URL(match[0])); } });
      });
      const headers = { "Content-Type": "application/json", Origin: url.origin, "X-Axiom-Session": url.hash.slice(1) };
      const call = async (action, payload) => {
        const response = await fetch(url.origin + "/api/tool-behavior" + (action ? "/" + action : ""),
          payload === undefined ? { headers, signal: AbortSignal.timeout(10000) }
            : { method: "POST", headers, body: JSON.stringify(payload), signal: AbortSignal.timeout(10000) });
        return { status: response.status, body: await response.json() };
      };
      const denied = await fetch(url.origin + "/api/tool-behavior", { signal: AbortSignal.timeout(10000) });
      assert.equal(denied.status, 403);
      assert.equal((await call("")).body.schemas.length, command("inspect").schemas.length);
      assert.equal((await call("preview", request)).body.approval, expected.approval);
      const applied = await call("apply", { ...request, approval: expected.approval });
      assert.equal(applied.status, 200);
      assert.equal((await call("revert", { bundle: applied.body.bundle })).status, 200);
      assert.equal(read(toolBehaviorPath).text, original.text);
    });
  } finally {
    if (server && server.exitCode === null) { server.kill(); await new Promise(resolve => server.once("exit", resolve)); }
    const resolved = fs.realpathSync(fixture), safeBase = fs.realpathSync(base) + path.sep;
    if (!resolved.startsWith(safeBase) || !path.basename(resolved).startsWith("fixture-")) throw new Error("Unexpected test cleanup target");
    fs.rmSync(resolved, { recursive: true, force: true });
  }
});
