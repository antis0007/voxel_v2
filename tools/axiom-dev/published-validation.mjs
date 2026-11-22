import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { realpath, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { captureSnapshot, digest, nativeCheckInput } from "./parity-snapshot.mjs";
import { loadWorkflows } from "./workflow-store.mjs";
import { createAgentLane } from "./agent-lane.mjs";
import { ensureDirectory, root, state } from "./workspace.mjs";

const app = "minecraft_style_voxel_prototype";
const directory = path.join(state, "published-validation");
const latest = path.join(directory, "latest.json");
const renameWait = new Int32Array(new SharedArrayBuffer(4));
const packages = [app, "engine_voxel", "engine_chip"];
const coverage = {
  L147: { tests: [["engine_voxel", "lava_and_water_make_basalt_and_steam"], [app, "environment_full_queue_retains_steam"]],
    gap: "Needs an asserted sparse-world contact that checks both products and retained liquid volume." },
  "A01-08": { tests: [[app, "world::pickups::tests::"]], gap: "Needs the codex receipt and host/guest save-reload checked after collection." },
  "A01-07": { tests: [[app, "headless_inventory_hover_shows_material_and_service_action"]], gap: "Headless egui verifies the inventory hover. In-world visual review remains open." },
  L016: { tests: [[app, "world::adventure::night::tests::"]], gap: "Needs an asserted fixture for opaque footing and the 24-block distance at final pack placement." },
  "A15-04": { tests: [[app, "world::factory::quarry::tests::"]], gap: "Needs a live quarry/drill path and machine-panel agreement through save/reload." },
  "A15-03": { tests: [["engine_chip", "firmware::tests::"]], gap: "Needs a cross-caller replay of the shared interpreter, including the instruction limit." },
  "QOL-20": { tests: [[app, "gpu_posed_vertices_match_cpu_pose", "--ignored"]], gap: "Needs a rendered orbital workload that exercises the new dispatch and CPU fallback.",
    performance: "Needs matched orbital pose-heavy frame captures with equivalent output." },
  "QOL-19": { tests: [[app, "headless_pause_log_export_recovers_and_excludes_player_identity"]], gap: "Headless egui exercises the Pause → Log export control. A full application crash and restart remains open." },
  "QOL-18": { tests: [[app, "headless_f3_attributes_budget_hitch_and_deferred_steps_retry"]], gap: "The headless check supplies deterministic deadline and frame telemetry. Live rendering under load remains open.",
    performance: "Needs repeated profiler-on/off runs on the same hitch workload." },
  L026: { gap: "The parity worker covers Crusher Screen. The panel preview and downstream gold furnace path need their own assertions." },
  "A15-01": { tests: [["engine_voxel", "packed_channels_allocate_independently_and_release_the_last_page"],
      [app, "torch_brick_cap_stops_spill_and_removal_reclaims_all_bytes"]],
    diagnostic: [app, "block_light_chunk_cost", "--ignored"],
    gap: "Needs an in-world save/reload and host/guest replay of independent plane allocation.",
    performance: "The existing light-cost probe compares flood algorithms. A matched pre/post plane allocation workload with byte and timing gates is still needed." },
};

export function publishedValidationStatus() {
  try { return JSON.parse(readFileSync(latest, "utf8")); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
}

export function publishedBenchmarkStatus() {
  try { return JSON.parse(readFileSync(path.join(directory, "benchmarks", "latest.json"), "utf8")); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
}

export function replaceValidationReceipt(target, json) {
  const temporary = target + "." + randomUUID() + ".tmp";
  writeFileSync(temporary, json, { flag: "wx" });
  const deadline = Date.now() + 1500;
  try {
    for (;;) {
      try { renameSync(temporary, target); return; }
      catch (error) {
        if (!["EPERM", "EACCES", "EBUSY"].includes(error.code) || Date.now() >= deadline) throw error;
        Atomics.wait(renameWait, 0, 0, 40);
      }
    }
  } finally { rmSync(temporary, { force: true }); }
}

const lane = createAgentLane("published-validation-lane.lock");
export const publishedValidationLane = lane.agentLane;

async function command(program, args, cwd, signal, onLine) {
  const child = spawn(program, args, { cwd, signal, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, CARGO_INCREMENTAL: "0" } });
  let output = "", launchError;
  for (const stream of [child.stdout, child.stderr]) createInterface({ input: stream }).on("line", line => {
    onLine?.(line);
    output = (output + line + "\n").slice(-96_000);
  });
  child.once("error", error => { launchError = error; });
  const code = await new Promise(resolve => child.once("close", resolve));
  if (launchError) throw launchError;
  return { code, output };
}

async function run(resume, recheck) {
  if (resume && !/^\d{4}-\d\d-\d\dT\d\d-\d\d-\d\d-\d{3}Z$/.test(resume)) {
    throw new Error("Pass the recorded validation run ID to resume it.");
  }
  if (recheck && !coverage[recheck]?.tests?.length) throw new Error("Choose a published mission with registered native checks.");
  const owner = lane.agentLane();
  if (owner && !owner.alive) lane.recoverAgentLane();
  const lease = lane.acquireAgentLane("Published mission validation");
  const runId = resume ?? new Date().toISOString().replace(/[:.]/g, "-");
  const folder = path.join(ensureDirectory(directory), runId);
  const source = path.join(folder, "source");
  let report;
  const save = () => {
    report.updatedAt = new Date().toISOString();
    const json = JSON.stringify(report, null, 2);
    for (const target of [path.join(folder, "report.json"), latest]) replaceValidationReceipt(target, json);
  };
  try {
    ensureDirectory(folder);
    const binaries = new Map();
    if (resume) {
      report = JSON.parse(readFileSync(path.join(folder, "report.json"), "utf8"));
      for (const { pkg, file, sha } of report.binaries ?? []) {
        if (!packages.includes(pkg) || !file.startsWith(pkg + "-") || !/^[\w-]+(?:\.exe)?$/.test(file)) {
          throw new Error("Invalid recorded native executable.");
        }
        const executable = path.join(root, "target", "release-fast", "deps", file);
        if (digest(readFileSync(executable)) !== sha) throw new Error("Recorded executable changed: " + file);
        binaries.set(pkg, executable);
      }
      if (!binaries.size) throw new Error("This run has no recorded native executable to resume.");
      if (report.blocker?.includes("rename")) delete report.blocker;
      report.resumedFromVerifiedBinaries = true;
    } else {
      const previous = recheck ? publishedValidationStatus() : undefined;
      const rows = loadWorkflows().filter(m => m.phase === "Complete" && m.followUpOf)
        .flatMap(m => {
          const id = m.objective.split(" · ")[0];
          return Object.hasOwn(coverage, id) ? [{ id, mission: m.id, title: m.objective.split("\n")[0],
            bundles: m.steps.map(step => step.bundleId).filter(Boolean),
            published: m.steps.flatMap(step => step.candidate?.evidence.files.map(file => ({ path: file.path, sha: file.after })) ?? []),
            gameplay: { status: id === "L026" ? "Parity worker" : coverage[id].tests ? "Pending" : "Blocked", detail: coverage[id].gap, checks: [] },
            performance: { status: "Blocked", detail: coverage[id].performance ?? "No registered mission-specific workload and comparable performance baseline." } }] : [];
        });
      if (!rows.length) throw new Error("No published missions match the native validation coverage.");
      if (recheck && !rows.some(row => row.id === recheck)) throw new Error("The mission is not published.");
      for (const [index, row] of rows.entries()) {
        if (!recheck || row.id === recheck) continue;
        const prior = previous?.rows?.find(item => item.mission === row.mission
          && JSON.stringify(item.bundles) === JSON.stringify(row.bundles));
        if (prior) rows[index] = { ...prior, evidenceRun: prior.evidenceRun ?? previous.runId, evidenceSource: prior.evidenceSource ?? previous.source };
      }
      report = { runId, phase: "Capturing source", rows, recheck };
      save();
      const snapshot = await captureSnapshot(source, AbortSignal.timeout(180_000), nativeCheckInput);
      report.source = digest(JSON.stringify([...snapshot.inputs]));
      writeFileSync(path.join(folder, "source-hashes.json"), JSON.stringify(Object.fromEntries(snapshot.inputs), null, 2));
      for (const row of rows) if (!recheck || row.id === recheck) {
        row.changedSincePublication = row.published.filter(f => snapshot.inputs.get(f.path) !== f.sha).map(f => f.path);
        row.evidenceRun = runId;
        row.evidenceSource = report.source;
      }
      report.phase = "Building native test executables";
      save();
      const compilerErrors = [];
      const selectedPackages = recheck ? [...new Set(coverage[recheck].tests.map(([pkg]) => pkg))] : packages;
      const build = await command("cargo", ["test", "--profile", "release-fast", "--offline", "--locked", "--jobs", "2",
        "--no-run", "--message-format=json", "--target-dir", path.join(root, "target"), ...selectedPackages.flatMap(pkg => ["-p", pkg])],
      source, AbortSignal.timeout(35 * 60_000), line => {
        let record;
        try { record = JSON.parse(line); } catch { return; }
        if (record.reason === "compiler-message" && record.message.level === "error") {
          const location = record.message.spans.find(span => span.is_primary);
          if (compilerErrors.length < 8) compilerErrors.push((location ? location.file_name + ":" + location.line_start + " · " : "") + record.message.message);
        }
        if (record.reason === "compiler-artifact" && record.profile.test && record.executable && packages.includes(record.target.name)) {
          binaries.set(record.target.name, record.executable);
        }
      });
      writeFileSync(path.join(folder, "build.log"), build.output);
      if (build.code !== 0) {
        report.blocker = compilerErrors.join("\n") || build.output.split("\n").filter(line => line.startsWith("error")).slice(0, 4).join("\n") ||
          "Native test build failed. See build.log.";
        save();
        if (!binaries.size) {
          report.phase = "Build blocked";
          for (const row of rows) if (row.gameplay.status === "Pending") row.gameplay.status = "Blocked";
          save();
          return report;
        }
      }
      report.binaries = Array.from(binaries, ([pkg, file]) => ({ pkg, file: path.basename(file), sha: digest(readFileSync(file)) }));
      save();
    }
    const cwd = resume ? folder : source;
    const listed = new Map();
    for (const [pkg, executable] of binaries) {
      const names = [];
      const result = await command(executable, ["--list", "--format", "terse"], cwd, AbortSignal.timeout(60_000), line => {
        if (line.endsWith(": test")) names.push(line.slice(0, -6));
      });
      if (result.code === 0) listed.set(pkg, names);
    }
    async function check(id, [pkg, selector, mode], kind) {
      if (!listed.get(pkg)?.some(name => name.includes(selector))) return { selector, status: "Blocked",
        detail: binaries.has(pkg) ? "The native harness did not list a matching test." : "The native test executable did not build." };
      const result = await command(binaries.get(pkg), [selector, "--test-threads=1", "--nocapture", ...(mode ? [mode] : [])],
        cwd, AbortSignal.timeout(300_000));
      const log = id + "-" + kind + "-" + digest(selector).slice(0, 10) + ".log";
      writeFileSync(path.join(folder, log), result.output);
      const [count] = [...result.output.matchAll(/(\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; \d+ filtered out/g)].slice(-1);
      const unavailable = !count || (!Number(count[1]) && !Number(count[2])) || /No (?:suitable )?(?:GPU|adapter)|adapter unavailable|skipping GPU/i.test(result.output);
      return { selector, status: unavailable ? "Blocked" : result.code === 0 ? "Passed" : "Failed",
        passed: Number(count?.[1] ?? 0), failed: Number(count?.[2] ?? 0), log };
    }
    for (const row of report.rows.filter(row => !report.recheck || row.id === report.recheck)) {
      report.phase = "Checking " + row.id;
      save();
      for (const test of coverage[row.id].tests ?? []) {
        const previous = row.gameplay.checks.find(result => result.selector === test[1]);
        if (previous && previous.detail !== "The native harness did not list a matching test.") continue;
        if (previous) row.gameplay.checks.splice(row.gameplay.checks.indexOf(previous), 1);
        try { row.gameplay.checks.push(await check(row.id, test, "native")); }
        catch (error) { row.gameplay.checks.push({ selector: test[1], status: "Blocked", detail: error.message }); }
      }
      const statuses = row.gameplay.checks.map(check => check.status);
      if (statuses.includes("Failed")) row.gameplay.status = "Failed";
      else if (statuses.includes("Passed")) row.gameplay.status = "Partial";
      else if (row.gameplay.status === "Pending") row.gameplay.status = "Blocked";
      if (coverage[row.id].diagnostic && !row.performance.diagnostic) {
        try {
          row.performance.diagnostic = await check(row.id, coverage[row.id].diagnostic, "performance");
          if (row.performance.diagnostic.status === "Passed") row.performance.status = "Partial";
        }
        catch (error) { row.performance.diagnostic = { status: "Blocked", detail: error.message }; }
      }
      save();
    }
    report.phase = "Checks finished";
    save();
    return report;
  } catch (error) {
    if (!report) throw error;
    report.phase = "Blocked"; report.blocker = error.message; save();
    return report;
  } finally {
    try {
      const resolved = await realpath(source).catch(() => undefined);
      if (resolved === source && path.dirname(resolved) === folder) await rm(resolved, { recursive: true, force: true, maxRetries: 3 });
    } finally { lane.releaseAgentLane(lease); }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [mode, mission] = process.argv.slice(2);
  await run(mode === "--recheck" ? undefined : mode, mode === "--recheck" ? mission : undefined)
    .then(report => {
      console.log(JSON.stringify({ runId: report.runId, phase: report.phase, blocker: report.blocker }));
      if (report.blocker || report.rows.some(row => row.gameplay.checks.some(check => check.status !== "Passed"))) process.exitCode = 1;
    })
    .catch(error => { process.stderr.write(error.message + "\n"); process.exitCode = 1; });
}
