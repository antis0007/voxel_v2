import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn, spawnSync } from "node:child_process";

test("material progression uses real native checks and reversible Forge publication", async t => {
  const source = fileURLToPath(new URL("../../", import.meta.url));
  const base = path.join(source, "target/material-progression-tests");
  fs.mkdirSync(base, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(base, "fixture-"));
  let server;
  try {
    for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "assets/materials.ron", "assets/mining.ron",
      "assets/tools/materials.ron", "assets/tools/parts.ron", "crates/app/src/world/mining.rs"]) {
      fs.mkdirSync(path.dirname(path.join(fixture, file)), { recursive: true });
      fs.copyFileSync(path.join(source, file), path.join(fixture, file));
    }
    for (const folder of ["tools/axiom-dev", "crates/core", "crates/content", "crates/audio", "crates/tools", "target/axiom-dev/native-validation"]) {
      fs.cpSync(path.join(source, folder), path.join(fixture, folder), { recursive: true });
    }
    fs.mkdirSync(path.join(fixture, "saves"));
    fs.writeFileSync(path.join(fixture, "saves/world.bin"), "player save sentinel");
    const module = name => import(pathToFileURL(path.join(fixture, "tools/axiom-dev", name)));
    const { materialProgressionCommand: command, progressionPath } = await module("material-progression.mjs");
    const { read, save } = await module("workspace.mjs");
    const original = read(progressionPath), initial = command("inspect", { materialId: "copper" });
    const request = { materialId: "copper", schemaId: "pickaxe", version: initial.version, miningLevel: 2 };

    await t.test("preview validates natively without publishing and reports gained ore gates", () => {
      const preview = command("preview", request);
      assert.equal(preview.changed, true);
      for (const block of ["IronOre", "Hematite", "Magnetite", "GoldQuartz", "Crystal"]) assert(preview.impact.gained.some(row => row.block === block));
      assert(preview.impact.blocked.some(row => row.block === "MithrilBlock"));
      assert.equal(preview.files.length, 1);
      assert.equal(read(progressionPath).version, original.version);
      assert.deepEqual(read(progressionPath).backups, original.backups);
    });
    await t.test("invalid intent, altered approval and native-invalid material cannot publish", () => {
      const preview = command("preview", request);
      for (const edit of [{ miningLevel: -1 }, { miningLevel: 256 }, { miningLevel: 1.5 }, { miningLevel: "2" }, { schemaId: "invented" }, { materialId: "invented" }, { renamedId: "new" }]) {
        assert.throws(() => command("preview", { ...request, ...edit }));
      }
      assert.throws(() => command("apply", { ...request, miningLevel: 3, approval: preview.approval }), /exact material change/);
      const file = path.join(fixture, progressionPath);
      fs.writeFileSync(file, original.text.replace("durability: 35.0", "durability: -1.0"));
      assert.throws(() => command("preview", { ...request, version: command("inspect", { materialId: "copper" }).version }), /Native validation rejected/);
      fs.writeFileSync(file, original.text);
      assert.equal(read(progressionPath).version, original.version);
      assert.deepEqual(read(progressionPath).backups, original.backups);
    });
    await t.test("apply changes one existing tier and revert restores exact source", () => {
      const before = command("inspect").materials, preview = command("preview", request);
      const applied = command("apply", { ...preview.request, approval: preview.approval });
      const after = command("inspect").materials;
      assert.deepEqual(after.filter(row => row.id !== "copper"), before.filter(row => row.id !== "copper"));
      assert.equal(after.find(row => row.id === "copper").miningLevel, 2);
      assert.throws(() => command("apply", { ...preview.request, approval: preview.approval }), /changed/);
      command("revert", { bundle: applied.bundle });
      assert.equal(read(progressionPath).version, original.version);
      assert.equal(fs.readFileSync(path.join(fixture, "saves/world.bin"), "utf8"), "player save sentinel");
    });
    await t.test("material, gate, schema, adapter and native changes invalidate reviews", () => {
      for (const file of [progressionPath, "assets/mining.ron", "assets/tools/parts.ron", "crates/app/src/world/mining.rs", "tools/axiom-dev/material-progression-data.mjs"]) {
        const preview = command("preview", request), location = path.join(fixture, file), before = fs.readFileSync(location);
        fs.writeFileSync(location, Buffer.concat([before, Buffer.from("\n")]));
        assert.throws(() => command("apply", { ...preview.request, approval: preview.approval }), /changed/);
        fs.writeFileSync(location, before);
      }
    });
    await t.test("no-op has no approval and another schema changes the reviewed impact", () => {
      const noChange = command("preview", { ...request, miningLevel: initial.materials[0].miningLevel });
      assert.equal(noChange.changed, false);
      assert.equal(noChange.approval, null);
      assert.throws(() => command("apply", { ...noChange.request, approval: null }), /exact material change/);
      const preview = command("preview", request);
      assert.throws(() => command("apply", { ...preview.request, schemaId: "axe", approval: preview.approval }), /exact material change/);
      const bow = command("preview", { ...request, schemaId: "shortbow" });
      assert.equal(bow.impact.materialAffectsMiningLevel, false);
      assert(bow.appliesToSchemas.some(row => row.id === "pickaxe"));
    });
    await t.test("revert refuses to replace a later material edit", () => {
      const preview = command("preview", request), applied = command("apply", { ...preview.request, approval: preview.approval });
      const current = read(progressionPath);
      const newer = save(progressionPath, { version: current.version, text: current.text + "\n// newer edit\n" });
      assert.throws(() => command("revert", { bundle: applied.bundle }), /changed/);
      assert.equal(read(progressionPath).version, newer.version);
      save(progressionPath, { version: newer.version, text: current.text });
      command("revert", { bundle: applied.bundle });
    });
    await t.test("stale validators allow inspection but never preview or publication", () => {
      const receipt = path.join(fixture, "target/axiom-dev/native-validation/receipt.json"), originalReceipt = fs.readFileSync(receipt);
      const changed = JSON.parse(originalReceipt); changed.source = "stale";
      fs.writeFileSync(receipt, JSON.stringify(changed));
      assert.equal(command("inspect", { materialId: "copper" }).materials[0].miningLevel, 1);
      assert.throws(() => command("preview", request), /missing or stale/);
      assert.equal(read(progressionPath).version, original.version);
      fs.writeFileSync(receipt, originalReceipt);
    });
    await t.test("simple CLI preview matches shared native-validated command", () => {
      const cli = path.join(fixture, "tools/axiom-dev/material-progression-cli.mjs");
      const result = spawnSync(process.execPath, [cli, "preview", "copper", "2"], { encoding: "utf8", windowsHide: true, timeout: 30000 });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), command("preview", request));
      for (const args of [["preview", "copper", "2.5"], ["apply", "copper", "2"], ["publish"]]) {
        assert.equal(spawnSync(process.execPath, [cli, ...args], { encoding: "utf8", windowsHide: true, timeout: 30000 }).status, 1);
      }
    });
    await t.test("HTTP preserves session/origin checks and shares preview/apply/revert", async () => {
      server = spawn(process.execPath, [path.join(fixture, "tools/axiom-dev/server.mjs")], { cwd: fixture, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
      const url = await new Promise((resolve, reject) => {
        let text = "", errors = "";
        const timeout = setTimeout(() => reject(new Error("Forge fixture startup timed out: " + errors)), 30000);
        server.stderr.on("data", data => { errors = (errors + data).slice(-2000); });
        server.on("error", error => { clearTimeout(timeout); reject(error); });
        server.on("exit", code => { clearTimeout(timeout); reject(new Error("Forge fixture exited: " + code + " " + errors)); });
        server.stdout.on("data", data => { text += data; const match = text.match(/http:\/\/127\.0\.0\.1:\d+\/#\w+/); if (match) { clearTimeout(timeout); resolve(new URL(match[0])); } });
      });
      const headers = { "Content-Type": "application/json", Origin: url.origin, "X-Axiom-Session": url.hash.slice(1) };
      const call = async (action, payload, override = headers) => {
        const response = await fetch(url.origin + "/api/material-progression" + (action ? "/" + action : ""),
          payload === undefined ? { headers: override, signal: AbortSignal.timeout(10000) }
            : { method: "POST", headers: override, body: JSON.stringify(payload), signal: AbortSignal.timeout(10000) });
        return { status: response.status, body: await response.json() };
      };
      assert.equal((await call("", undefined, {})).status, 403);
      assert.equal((await call("preview", request, { ...headers, Origin: "https://example.com" })).status, 403);
      assert.equal((await call("preview", { ...request, version: "stale" })).status, 400);
      assert.deepEqual((await call("")).body.materials, command("inspect").materials);
      const preview = await call("preview", request);
      assert.equal(preview.status, 200);
      assert.equal(preview.body.approval, command("preview", request).approval);
      const applied = await call("apply", { ...preview.body.request, approval: preview.body.approval });
      assert.equal(applied.status, 200);
      assert.equal((await call("revert", { bundle: applied.body.bundle })).status, 200);
      assert.equal(read(progressionPath).version, original.version);
    });
  } finally {
    if (server && server.exitCode === null) { server.kill(); await new Promise(resolve => server.once("exit", resolve)); }
    const resolved = fs.realpathSync(fixture), safeBase = fs.realpathSync(base) + path.sep;
    if (!resolved.startsWith(safeBase) || !path.basename(resolved).startsWith("fixture-")) throw new Error("Unexpected fixture cleanup target");
    fs.rmSync(resolved, { recursive: true, force: true });
  }
});
