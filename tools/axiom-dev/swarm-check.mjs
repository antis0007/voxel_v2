// Shared compile check for swarm agents.
//
//   node tools/axiom-dev/swarm-check.mjs [package]     (default: minecraft_style_voxel_prototype)
//
// Agents used to run their own `cargo check` one after another behind the build lock (about 3 minutes each)
// and read 700+ warnings to find their errors, often truncated. This asks the running swarm supervisor,
// which batches every agent waiting at that moment into ONE cargo run, and prints only the errors. If no
// supervisor answers, it runs cargo itself with the same filter. Agents cannot edit tools/**, so the
// command they are allowed to run is fixed.
import { spawn } from "node:child_process";
import http from "node:http";
import { pathToFileURL } from "node:url";

export const APP = "minecraft_style_voxel_prototype";
export const validPackage = p => (/^[\w-]{1,64}$/.test(p ?? "") ? p : APP);

// One cargo check, reduced to what an agent needs: ok, error lines, warning count and a short text.
export function runCheck(pkg, { root = process.cwd(), env = process.env, timeoutMs = 25 * 60_000 } = {}) {
  return new Promise(resolve => {
    let out = "";
    const child = spawn("cargo", ["check", "-p", pkg, "--message-format", "short"], { cwd: root, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    const timer = setTimeout(() => child.kill(), timeoutMs);
    const take = c => { out = (out + c).slice(-400_000); };
    child.stdout.setEncoding("utf8").on("data", take);
    child.stderr.setEncoding("utf8").on("data", take);
    const done = code => {
      clearTimeout(timer);
      const lines = out.split(/\r?\n/);
      const errors = [...new Set(lines.filter(l => /: error(\[E\d+\])?: /.test(l) || (/^error(\[E\d+\])?: /.test(l) && !/could not compile|aborting due to/.test(l))).map(l => l.trim()))];
      const warnings = lines.filter(l => /: warning: /.test(l)).length;
      const ok = code === 0;
      const text = ok ? `OK: ${pkg} compiles (${warnings} warnings not shown).`
        : errors.length ? `FAILED: ${errors.length} error(s) in ${pkg}:\n${errors.slice(0, 40).join("\n")}\nErrors in files you did not edit are peers' work in progress: do not fix them.`
        : `FAILED (no compiler error lines). cargo said:\n${lines.filter(Boolean).slice(-15).join("\n")}`;
      resolve({ ok, errors, warnings, text, at: Date.now() });
    };
    child.on("error", e => { clearTimeout(timer); resolve({ ok: false, errors: [], warnings: 0, text: "FAILED: could not start cargo: " + e.message, at: Date.now() }); });
    child.on("close", done);
  });
}

// Content tables are parsed and validated only at runtime, so cargo check passed while the game panicked at
// start (a Fallout row without sim_as; reaction rows naming a Plasma block that does not exist). This loads
// every validated table once through engine_content's check_tables example.
export function runTables({ root = process.cwd(), env = process.env, timeoutMs = 15 * 60_000 } = {}) {
  return new Promise(resolve => {
    let out = "";
    const child = spawn("cargo", ["run", "-q", "-p", "engine_content", "--example", "check_tables"], { cwd: root, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    const timer = setTimeout(() => child.kill(), timeoutMs);
    child.stdout.setEncoding("utf8").on("data", c => { out = (out + c).slice(-100_000); });
    child.stderr.setEncoding("utf8").on("data", c => { out = (out + c).slice(-100_000); });
    const finish = code => {
      clearTimeout(timer);
      const failures = out.split(/\r?\n/).filter(l => l.startsWith("FAILED ")).map(l => l.slice(7, 600));
      const ran = /^ok |^FAILED /m.test(out);
      // The example failing to build is a compile problem the app check reports, not a table failure.
      resolve({ ok: code === 0 || !ran, failures, ran });
    };
    child.on("error", () => { clearTimeout(timer); resolve({ ok: true, failures: [], ran: false }); });
    child.on("close", finish);
  });
}

function viaSupervisor(pkg, port) {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: "127.0.0.1", port, path: "/check?pkg=" + encodeURIComponent(pkg), headers: { host: `127.0.0.1:${port}` } }, res => {
      if (res.statusCode !== 200) { res.resume(); reject(new Error("status " + res.statusCode)); return; }
      let body = ""; res.setEncoding("utf8"); res.on("data", c => { body += c; }); res.on("end", () => resolve(body));
    });
    req.setTimeout(30 * 60_000, () => req.destroy(new Error("timeout")));
    req.on("error", reject);
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const pkg = validPackage(process.argv[2]);
  const port = Number(process.env.AXIOM_SWARM_PORT || 4377);
  let text;
  try { text = await viaSupervisor(pkg, port); }
  catch { text = (await runCheck(pkg)).text; }
  console.log(text);
  process.exitCode = text.startsWith("OK") ? 0 : 1;
}
