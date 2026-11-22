import { appPort, appToken, appRevision, backendRevision } from "./app-lifecycle.mjs";
import fabricationMetadata from "./fabrication-contract.mjs";
import { materialProgressionCommand } from "./material-progression.mjs";
import { toolBehaviorCommand } from "./tool-behavior.mjs";
import { portalCommand } from "./portal-service.mjs";
import { serveFoundryAsset, serveFoundryApi } from "./foundry-service.mjs";
import { serveNormalLab } from "./normal-lab-server.mjs";
import { FAMILIES, STATUSES, brief, loadLedger, writeStatus } from "./parity-data.mjs";
import { parityEvidence, parityVerification, parityWorkChecks, parityWorkProgress, parityWorkStatus, startParityRefreshScheduler, startParityWork, stopParityWork, recordParityWork } from "./parity-work.mjs";
import http from "node:http";
import { renderToolArt } from "./native-art.mjs";
import { renderSavedToolArt } from "./saved-tool-art.mjs";
import { createHash, randomBytes } from "node:crypto";
import { readFileSync, writeFileSync, renameSync, rmSync } from "node:fs";
import { agentLane, createAgentLane } from "./agent-lane.mjs";
import path from "node:path";
import { files, read, save, history, preview, modelBytes, usages, maxAsset, readNavigation, writeNavigation, state, ensureDirectory } from "./workspace.mjs";
import { engineRequest, enginePreset, engineSample, startEngineSession } from "./engine-service.mjs";
import { readWorkbench, writeWorkbench, writeSpriteDraft } from "./workspace.mjs";
import * as game from "./game.mjs";
import { sparkPreview, spellPath } from "./forge-spark.mjs";
import { searchProject, notesIndex, createNote, invalidateKnowledge } from "./project-knowledge.mjs";
import { buildGameGraph } from "./game-graph.mjs";
import { anchorStudio } from "./anchor-link.mjs";
import { foodWorkflowCatalog, inspectFoodWorkflow, previewFoodWorkflow, applyFoodWorkflow,
  previewFoodWithdrawal, withdrawFoodRecipe } from "./food-workflow.mjs";
import { checkAgentRegression, startImprovementLoop, decisionReceipts, activeAgentPaths, agentStatus, cancelAgent, cancelImprovement, improvementStatus, recentReceipts,
  recheckAgent, reviewAgent, startAgent, startImprovement } from "./agent-runner.mjs";
import { blocksPlaytest, refineDesign } from "./design-refinement.mjs";
import { cachedMod, inspectClass, inspectMod, inspectModel, inspectTexture, maxJarBytes } from "./mod-evidence.mjs";
import { canAssist, maxSources, maxTaskLength, maxExcerpt, maxPackReference } from "./agent-contract.mjs";
import { collectContext } from "./agent-context.mjs";
import { decisionBundles, inspectBundle, publishBundle, recentBundles, revertBundle } from "./agent-bundles.mjs";
import { readMaterial, saveMaterial, suggestMaterial } from "./material-service.mjs";
import { suggestRecipe } from "./craft-suggestions.mjs";
import { decisions, proposeNext } from "./agent-decisions.mjs";
import { changeBacklog, readBacklog } from "./backlog-store.mjs";
import { parityMissionProgress, workflowCommand, workflowStatus, startWorkflowScheduler } from "./workflow.mjs";
import { recoverWorkflowStore } from "./workflow-store.mjs";
import { publishedBenchmarkStatus, publishedValidationLane, publishedValidationStatus } from "./published-validation.mjs";
import { wandSpells, evaluateBoth, optimizeWand, surveyWands, suggestWand, suggestBalance, listBuilds, saveBuild, deleteBuild, draftStatus, editDraft, resetDraft, applyDraft, bestiary } from "./wand-lab-service.mjs";

// JSON can require six wire bytes per UTF-16 code unit (e.g. \u0000).
// Keep transport limits above valid semantic inputs, including twelve mission steps.
const taskBytes = 6 * maxTaskLength;
// Per step: 600 query units; five 512-unit target paths, safe-integer line,
// SHA-256 revision and JSON keys/punctuation. Keep existing contract headroom.
const workflowContextBytes = 12 * (6 * 600 + maxSources * (6 * 512 + 16 + 64 + 64));
const workflowBytes = taskBytes + 6 * 12 * (1600 + 700 + 120) + workflowContextBytes + 32_768;
const agentBytes = 6 * (maxTaskLength + maxSources * maxExcerpt + maxPackReference) + 32_768;
const token = process.argv.includes("--check-ui-imports") ? "" : appToken();
const staticFiles = {
  "/swarm-frame.css": ["swarm-frame.css", "text/css"],
  "/app-updates.mjs": ["app-updates.mjs", "text/javascript"],
  "/workflow-panel.mjs": ["workflow-panel.mjs", "text/javascript"],
  "/tool-capabilities.mjs": ["tool-capabilities.mjs", "text/javascript"],
  "/catalog-contracts.mjs": ["catalog-contracts.mjs", "text/javascript"],
  "/coding-actions.mjs": ["coding-actions.mjs", "text/javascript"],
  "/portal-panel.mjs": ["portal-panel.mjs", "text/javascript"],
  "/portal-operations.mjs": ["portal-operations.mjs", "text/javascript"],
  "/portal-style.css": ["portal-style.css", "text/css"],
  "/tool-behavior-panel.mjs": ["tool-behavior-panel.mjs", "text/javascript"],
  "/": ["index.html", "text/html"],
  "/wand-lab": ["wand-lab.html", "text/html"],
  "/parity": ["parity.html", "text/html"],
  "/parity-ui.mjs": ["parity-ui.mjs", "text/javascript"],
  "/parity.css": ["parity.css", "text/css"],
  "/forge-shell.mjs": ["forge-shell.mjs", "text/javascript"],
  "/forge-shell.css": ["forge-shell.css", "text/css"],
  "/wand-lab-shell.mjs": ["wand-lab-shell.mjs", "text/javascript"],
  "/wand-lab-ui.mjs": ["wand-lab-ui.mjs", "text/javascript"],
  "/wand-lab.css": ["wand-lab.css", "text/css"],
  "/wand-lab-link.mjs": ["wand-lab-link.mjs", "text/javascript"],
  "/style.css": ["style.css", "text/css"],
  "/devtools-qol.css": ["devtools-qol.css", "text/css"],
  "/command-palette.mjs": ["command-palette.mjs", "text/javascript"],
  "/panel-organizer.mjs": ["panel-organizer.mjs", "text/javascript"],
  "/app.mjs": ["app.mjs", "text/javascript"],
  "/backlog-panel.mjs": ["backlog-panel.mjs", "text/javascript"],
  "/backlog.css": ["backlog.css", "text/css"],
  "/boot.mjs": ["boot.mjs", "text/javascript"],
  "/inspector.mjs": ["inspector.mjs", "text/javascript"],
  "/syntax.mjs": ["syntax.mjs", "text/javascript"],
  "/links.mjs": ["links.mjs", "text/javascript"],
  "/playtest.mjs": ["playtest.mjs", "text/javascript"],
  "/layout-editor.mjs": ["layout-editor.mjs", "text/javascript"],
  "/playtest-worlds.mjs": ["playtest-worlds.mjs", "text/javascript"],
  "/engine-panel.mjs": ["engine-panel.mjs", "text/javascript"],
  "/native-art-panel.mjs": ["native-art-panel.mjs", "text/javascript"],
  "/forge-spark.mjs": ["forge-spark.mjs", "text/javascript"],
  "/spell-wiring.mjs": ["spell-wiring.mjs", "text/javascript"],
  "/normal-lab.mjs": ["normal-lab.mjs", "text/javascript"],
  "/sprite-editor.mjs": ["sprite-editor.mjs", "text/javascript"],
  "/sprite-color.mjs": ["sprite-color.mjs", "text/javascript"],
  "/sprite-sheet.mjs": ["sprite-sheet.mjs", "text/javascript"],
  "/sprite-usage.mjs": ["sprite-usage.mjs", "text/javascript"],
  "/sprite-composition.mjs": ["sprite-composition.mjs", "text/javascript"],
  "/pixel-grid.mjs": ["pixel-grid.mjs", "text/javascript"],
  "/quick-open.mjs": ["quick-open.mjs", "text/javascript"],
  "/targets.mjs": ["targets.mjs", "text/javascript"],
  "/graph-workspace.mjs": ["graph-workspace.mjs", "text/javascript"],
  "/graph-canvas.mjs": ["graph-canvas.mjs", "text/javascript"],
  "/game-map.mjs": ["game-map.mjs", "text/javascript"],
  "/graph-query.mjs": ["graph-query.mjs", "text/javascript"],
  "/graph-inspector.mjs": ["graph-inspector.mjs", "text/javascript"],
  "/system-board.mjs": ["system-board.mjs", "text/javascript"],
  "/material-editor.mjs": ["material-editor.mjs", "text/javascript"],
  "/material-properties.mjs": ["material-properties.mjs", "text/javascript"],
  "/graph-style.css": ["graph-style.css", "text/css"],
  "/idea-data.mjs": ["idea-data.mjs", "text/javascript"],
  "/idea-creator.mjs": ["idea-creator.mjs", "text/javascript"],
  "/catalog.mjs": ["catalog.mjs", "text/javascript"],
  "/catalog-data.mjs": ["catalog-data.mjs", "text/javascript"],
  "/fabrication-contract.mjs": ["fabrication-contract.mjs", "text/javascript"],
  "/workshop-planning.mjs": ["workshop-planning.mjs", "text/javascript"],
  "/fabrication-data.mjs": ["fabrication-data.mjs", "text/javascript"],
  "/texture-study.mjs": ["texture-study.mjs", "text/javascript"],
  "/mod-pack.mjs": ["mod-pack.mjs", "text/javascript"],
  "/catalog-icons.mjs": ["catalog-icons.mjs", "text/javascript"],
  "/quick-balance.mjs": ["quick-balance.mjs", "text/javascript"],
  "/model-studio.mjs": ["model-studio.mjs", "text/javascript"],
  "/model-view.mjs": ["model-view.mjs", "text/javascript"],
  "/voxel-model.mjs": ["voxel-model.mjs", "text/javascript"],
  "/pose-studio.mjs": ["pose-studio.mjs", "text/javascript"],
  "/catalog-style.css": ["catalog-style.css", "text/css"],
  "/food-item.mjs": ["food-item.mjs", "text/javascript"],
  "/food-creator.mjs": ["food-creator.mjs", "text/javascript"],
  "/tint-controls.mjs": ["tint-controls.mjs", "text/javascript"],
  "/biome-studio.mjs": ["biome-studio.mjs", "text/javascript"],
  "/deposit-studio.mjs": ["deposit-studio.mjs", "text/javascript"],
  "/habitat-editor.mjs": ["habitat-editor.mjs", "text/javascript"],
  "/slot-tools.mjs": ["slot-tools.mjs", "text/javascript"],
  "/context-menu.mjs": ["context-menu.mjs", "text/javascript"],
  "/canvas-view.mjs": ["canvas-view.mjs", "text/javascript"],
  "/workbench.css": ["workbench.css", "text/css"],
  "/craft-index.mjs": ["craft-index.mjs", "text/javascript"],
  "/craft-web.mjs": ["craft-web.mjs", "text/javascript"],
  "/craft-web-data.mjs": ["craft-web-data.mjs", "text/javascript"],
  "/craft-workbench.mjs": ["craft-workbench.mjs", "text/javascript"],
  "/agent-panel.mjs": ["agent-panel.mjs", "text/javascript"],
  "/agent-contract.mjs": ["agent-contract.mjs", "text/javascript"],
  "/recipe-wiring.mjs": ["recipe-wiring.mjs", "text/javascript"],
  "/advancement-wiring.mjs": ["advancement-wiring.mjs", "text/javascript"],
  "/recipe-wiring-data.mjs": ["recipe-wiring-data.mjs", "text/javascript"],
  "/recipe-blocks.mjs": ["recipe-blocks.mjs", "text/javascript"],
  "/wiring-commands.mjs": ["wiring-commands.mjs", "text/javascript"],
  "/wire-canvas.mjs": ["wire-canvas.mjs", "text/javascript"],
  "/recipe-wiring.css": ["recipe-wiring.css", "text/css"],
  "/fonts/tiny5.ttf": ["../../assets/fonts/Tiny5.ttf", "font/ttf"],
  "/fonts/pixelcode.ttf": ["../../assets/fonts/PixelCode.ttf", "font/ttf"],
};

function checkUiImports() {
  const checked = new Set();
  const issues = [];
  const base = "http://localhost";
  // Follow literal local imports in the boot graph without executing browser code.
  const imports = /(?:^\s*(?:import\s+(?:[\w*$,\s{}]+?\s+from\s+)?|export\s+(?:\{[^}]*\}|\*)\s+from\s+)|\bimport\s*\(\s*)["']((?:\.{1,2}\/|\/(?!\/))[^"']+)["']/gm;
  function visit(route, importedBy) {
    const asset = Object.hasOwn(staticFiles, route) && staticFiles[route];
    if (!asset || asset[1] !== "text/javascript") {
      issues.push({ route, importedBy, error: "No JavaScript route in server.mjs" });
      return;
    }
    if (checked.has(route)) return;
    checked.add(route);
    let source;
    try {
      source = readFileSync(new URL(asset[0], import.meta.url), "utf8");
    } catch (error) {
      issues.push({ route, importedBy, file: asset[0], error: error.code ?? error.message });
      return;
    }
    for (const [, specifier] of source.matchAll(imports)) {
      visit(new URL(specifier, base + route).pathname, route);
    }
  }
  visit("/boot.mjs", null);
  return { ok: issues.length === 0, checked: checked.size, issues };
}

function send(response, status, content, type = "application/json") {
  response.writeHead(status, { "Content-Type": type, "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff", "Referrer-Policy": "no-referrer",
    "Content-Security-Policy": "default-src 'self'; img-src 'self' blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'" });
  response.end(type === "application/json" ? JSON.stringify(content) : content);
}

async function rawBody(request, limit) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > limit) throw new Error("Request exceeds the editor size limit");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks, size);
}

async function body(request, limit = Math.ceil(maxAsset * 4 / 3) + 16384) {
  return JSON.parse((await rawBody(request, limit)).toString());
}

// A queued mission may be claimed by another panel, including an older server.
// Stop its actual lane owner; do not mistake a durable request for acknowledgement.
async function controlWorkflow(input, forwarded = false) {
  if (input?.workflowStorageVersion !== 2) throw Object.assign(new Error("Refresh the current workflow client before changing the epoch-2 queue."), { status: 409 });
  if (typeof input.id === "string" && workflowStatus(input.id).readOnly) throw Object.assign(new Error("Legacy history is read-only here. Create a scoped follow-up; retained workers are unchanged."), { status: 409 });
  if (!["pause", "cancel"].includes(input?.action) || typeof input.id !== "string") {
    return workflowCommand(input, game.status().logs);
  }
  const conflict = message => { throw Object.assign(new Error(message), { status: 409 }); };
  const mission = workflowStatus(input.id);
  if (mission.revision !== input.revision) conflict("Mission changed. Refresh before retrying.");
  if (mission.phase === "Interrupted") conflict("Mission is interrupted and has no verified live owner. Inspect and recover it before requesting a stop.");
  if (!["Running", "Planning"].includes(mission.phase)) return workflowCommand(input, game.status().logs);
  const owner = workflowStatus().lane;
  if (!owner?.alive || owner.id !== mission.lease) conflict("Mission owner changed. Refresh before retrying.");
  if (owner.pid === process.pid) return workflowCommand(input, game.status().logs);
  if (forwarded) conflict("Mission owner changed during stop routing. Refresh before retrying.");
  let session, endpoint;
  try {
    session = JSON.parse(readFileSync(path.join(state, "engine-session-" + owner.pid + ".json"), "utf8"));
    endpoint = new URL(session.origin);
    if (session.protocol !== 1 || session.pid !== owner.pid || !/^[a-f0-9]{48}$/.test(session.token)
        || endpoint.protocol !== "http:" || endpoint.hostname !== "127.0.0.1" || !endpoint.port
        || endpoint.username || endpoint.password || endpoint.pathname !== "/" || endpoint.search || endpoint.hash) {
      throw new Error("Invalid owner session");
    }
  } catch { conflict("Cannot identify the mission owner's local session. No stop was sent; inspect the owner panel."); }
  // Forward the original revision exactly once. The owner rechecks it under the ledger lock.
  let response, result;
  try {
    response = await fetch(new URL("/api/workflows", endpoint), {
      method: "POST", headers: { "X-Axiom-Session": session.token, "Content-Type": "application/json",
        "X-Axiom-Control-Forwarded": "1" }, body: JSON.stringify(input),
      redirect: "error", signal: AbortSignal.timeout(8000),
    });
    result = await response.json();
  } catch {
    throw Object.assign(new Error("Owner stop acknowledgement was not received. Check mission status before retrying; no automatic retry was made."), { status: 502 });
  }
  if (!response.ok) throw Object.assign(new Error(result.error ?? "Owner rejected the stop request."), { status: response.status });
  if (result.id !== input.id) conflict("Unexpected owner response. Inspect mission status before retrying.");
  return result;
}

// The swarm supervisor is a separate local process (AxiomSwarm.cmd). Reads are proxied the same
// way the supervisor already serves them on its own loopback port; control posts reach this
// function only after the panel's session-token check above.
const swarmPort = Number(process.env.AXIOM_SWARM_PORT) || 4377;
const swarmRoutes = [
  [/^\/$/, "GET", "text/html; charset=utf-8"],
  [/^\/state$/, "GET", "application/json"],
  [/^\/engine$/, "GET", "application/json"],
  [/^\/plan$/, "GET", "application/json"],
  [/^\/events$/, "GET", "text/event-stream"],
  [/^\/prompt\/[\w.-]{1,40}$/, "GET", "text/plain; charset=utf-8"],
  [/^\/log\/[\w.:%-]{1,120}$/, "GET", "application/json"],
  [/^\/diff\/[\w.:%-]{1,120}$/, "GET", "text/plain; charset=utf-8"],
  [/^\/ctl\/[\w./%-]{1,160}$/, "POST", "application/json"],
];
async function proxySwarm(request, response, url) {
  const headers = { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff", "Referrer-Policy": "no-referrer",
    "Content-Security-Policy": "frame-ancestors 'self'" };
  const rest = url.pathname.slice("/swarm".length);
  const route = swarmRoutes.find(([pattern, method]) => method === request.method && pattern.test(rest));
  if (!route || rest.includes("..")) return send(response, 404, { error: "Unknown swarm route" });
  try {
    const post = request.method === "POST";
    const target = new URL("http://127.0.0.1:" + swarmPort);
    target.pathname = rest;
    target.search = url.search;
    if (target.hostname !== "127.0.0.1" || target.port !== String(swarmPort)) return send(response, 400, { error: "Bad path" });
    if (rest === "/events") {
      const abort = new AbortController();
      request.on("close", () => abort.abort());
      const stream = await fetch(target, { redirect: "manual", signal: abort.signal });
      response.writeHead(200, { ...headers, "Content-Type": "text/event-stream" });
      for await (const chunk of stream.body) response.write(chunk);
      return response.end();
    }
    const upstream = await fetch(target, {
      method: request.method, redirect: "manual", signal: AbortSignal.timeout(10_000),
      headers: post ? { "x-swarm-ctl": "1", "content-type": "application/json" } : undefined,
      body: post ? await rawBody(request, 65_536) : undefined,
    });
    headers["Content-Type"] = route[2];
    response.writeHead(upstream.status, headers);
    response.end(Buffer.from(await upstream.arrayBuffer()));
  } catch {
    if (response.headersSent) return response.end();
    response.writeHead(502, { ...headers, "Content-Type": "text/html; charset=utf-8" });
    response.end("<!doctype html><meta charset=utf-8><body style='font:14px system-ui;padding:2rem;color:#ccc;background:#111'>"
      + "Swarm supervisor is not running. Start <code>AxiomSwarm.cmd</code>, then reopen this tab.");
  }
}

const fabricationVersion = createHash("sha256").update(JSON.stringify(fabricationMetadata)).digest("hex");
let origin;
let stopping = false;
let updating = false;
let loadedBackend;
let currentRevision;
let changedAt = 0;
const worker = createAgentLane("dev-server.lock");
const parityLane = createAgentLane("parity-lane.lock").agentLane;
const workerSession = path.join(state, "engine-session-" + process.pid + ".json");
const server = http.createServer(async (request, response) => {
  try {
    if (request.headers.host !== new URL(origin).host
      || (request.headers.origin && request.headers.origin !== origin)) {
      return send(response, 403, { error: "Local panel only" });
    }
    const url = new URL(request.url, origin);
    const relative = url.searchParams.get("path");
    if (request.method === "GET") {
      // Only the supervisor page itself is served without the session token; state, logs, diffs
      // and control requests are proxied after the token check below.
      if (url.pathname === "/swarm/") return proxySwarm(request, response, url);
      if (serveNormalLab(url.pathname, response)) return;
      if (serveFoundryAsset(url.pathname, response)) return;
      const asset = Object.hasOwn(staticFiles, url.pathname) && staticFiles[url.pathname];
      if (asset) return send(response, 200, readFileSync(new URL(asset[0], import.meta.url)), asset[1]);
    }
    if (request.headers["x-axiom-session"] !== token) return send(response, 403, { error: "Open the URL printed by the launcher" });
    if (url.pathname.startsWith("/swarm/")) return proxySwarm(request, response, url);
    if (url.pathname === "/api/app" && request.method === "GET") return send(response, 200, { pid: process.pid, origin, revision: currentRevision, updating, singleton: true });
    if (url.pathname === "/api/worker") {
      const lanes = [agentLane(), parityLane()].filter(owner => owner?.alive);
      const { running, building } = game.status();
      const activity = { pid: process.pid, lanes, running, building, appVersion: 2, updating };
      if (request.method === "GET") return send(response, 200, activity);
      if (request.method !== "POST") return send(response, 405, { error: "Use POST to stop the idle worker." });
      if (lanes.length || running || building) return send(response, 409, { ...activity, error: "Work is active. Stop or let it finish, then retry." });
      stopping = true;
      server.close();
      response.once("finish", () => {
        server.closeAllConnections();
        process.exit(0);
      });
      return send(response, 200, { ...activity, stopping: true });
    }
    if (stopping) return send(response, 503, { error: "The worker is stopping." });
    const clientSchema = request.headers["x-axiom-fabrication-schema"];
    if (clientSchema !== undefined && clientSchema !== fabricationVersion) {
      return send(response, 409, { error: "Developer catalog metadata changed. Reload Forge before editing." });
    }
    if (url.pathname === "/api/backlog") {
      try {
        if (request.method === "GET") return send(response, 200, readBacklog(relative));
        if (request.method === "POST") return send(response, 200, changeBacklog(await body(request, 65_536)));
        return send(response, 405, { error: "Use GET or POST for backlog tasks." });
      } catch (error) {
        return send(response, error.status ?? 400, { error: error.message });
      }
    }
    if (request.method === "GET" && serveFoundryApi(url, response)) return;
    if (request.method === "GET") {
      switch (url.pathname) {
        case "/api/portal-layout": return send(response, 200, portalCommand("inspect"));
        case "/api/files": {
          const refresh = url.searchParams.has("refresh");
          if (refresh) invalidateKnowledge();
          return send(response, 200, files(refresh));
        }
        case "/api/navigation": return send(response, 200, readNavigation());
        case "/api/parity": {
          const ledger = loadLedger();
          const missions = parityMissionProgress(ledger);
          const work = parityWorkProgress();
          const verification = new Map((await parityVerification(ledger)).map(({ id, currentNative, registered, evidence }) => [id, { currentNative, registered, evidence }]));
          const missionRevision = JSON.stringify([...missions.values()].map(({ id, revision, phase }) => [id, revision, phase]));
          const liveRevision = createHash("sha256").update(JSON.stringify([ledger.planningRevisions, ledger.packRevision, missionRevision, [...work], [...verification]])).digest("hex");
          if (url.searchParams.get("since") === liveRevision) return send(response, 200, { liveRevision, unchanged: true });
          return send(response, 200, { revision: ledger.revision, matrix: ledger.matrixRevision,
            owners: ledger.planningRevisions.owners, missionRevision, liveRevision, statuses: STATUSES,
            families: Object.fromEntries(Object.entries(FAMILIES).map(([key, family]) => [key, family.label])),
            rows: ledger.rows.map(row => ({ id: row.id, label: row.label, section: row.section, family: row.family,
              families: row.families, status: row.status, milestone: row.milestone, cost: row.cost, risks: row.risks,
              mapping: row.mapping, depends: row.depends, ready: row.readiness.ready, missing: row.readiness.missing,
              paths: row.paths, evidence: row.evidence, note: row.note, updated: row.updated, notes: row.notes,
              priority: row.priority, packJars: row.packSources.map(source => source.jar),
              verification: verification.get(row.id), mission: missions.get(row.id), work: work.get(row.id) })) });
        }
        case "/api/parity/work": return send(response, 200, parityWorkStatus(url.searchParams.get("id")));
        case "/api/parity/evidence": {
          const ledger = loadLedger();
          const row = ledger.rows.find(entry => entry.id === url.searchParams.get("id"));
          if (!row) return send(response, 404, { error: "Unknown feature ID" });
          return send(response, 200, { entries: await parityEvidence(row, ledger.planningRevisions),
            mission: parityMissionProgress(ledger, row.id).get(row.id), work: parityWorkStatus(row.id) });
        }
        case "/api/parity/describe": {
          const ledger = loadLedger();
          const row = ledger.rows.find(entry => entry.id === url.searchParams.get("id"));
          if (!row) return send(response, 404, { error: "Unknown feature ID" });
          return send(response, 200, { revision: ledger.revision, workChecks: parityWorkChecks(row.id).map(scenario => scenario.id),
            ...brief(row, ledger.systems, true) });
        }
        case "/api/workbench": return send(response, 200, readWorkbench());
        case "/api/workflows": {
          const id = url.searchParams.get("id") ?? undefined;
          const view = url.searchParams.get("view");
          if (id || view === "summary") return send(response, 200, workflowStatus(id, view === "summary"));
          let validation;
          try {
            const report = publishedValidationStatus();
            if (report) validation = { ...report, active: !!publishedValidationLane()?.alive,
              parity: parityWorkStatus("L026"), benchmarks: publishedBenchmarkStatus() };
          } catch (error) { validation = { phase: "Evidence unavailable", blocker: error.message }; }
          if (view === "validation") return send(response, 200, { validation });
          return send(response, 200, { ...workflowStatus(), validation });
        }
        case "/api/agent": return send(response, 200, agentStatus(url.searchParams.get("run") ?? undefined));
        case "/api/improvement": return send(response, 200, improvementStatus());
        case "/api/improvement/decisions": return send(response, 200, decisions.list());
        case "/api/agent-bundles": return send(response, 200, relative ? inspectBundle(relative) : recentBundles());
        case "/api/agent/receipts": return send(response, 200, recentReceipts());
        case "/api/search": return send(response, 200, searchProject(url.searchParams.get("q"), url.searchParams.has("fields")));
        case "/api/notes": return send(response, 200, notesIndex());
        case "/api/game-graph": return send(response, 200, buildGameGraph());
        case "/api/model": return send(response, 200, modelBytes(relative), "model/gltf-binary");
        case "/api/read": return send(response, 200, read(relative));
        case "/api/material-progression": return send(response, 200, materialProgressionCommand("inspect", relative ? { materialId: relative } : {}));
        case "/api/tool-behavior": return send(response, 200, toolBehaviorCommand("inspect", relative ? { schemaId: relative } : {}));
        case "/api/material": return send(response, 200, readMaterial(relative));
        case "/api/usages": return send(response, 200, usages(relative));
        case "/api/history": return send(response, 200, { text: history(relative, url.searchParams.get("id")).toString("utf8") });
        case "/api/preview": {
          const image = preview(relative);
          return send(response, 200, image.bytes, image.mime);
        }
        case "/api/game": return send(response, 200, game.status());
        case "/api/wand-lab/spells": return send(response, 200, await wandSpells(url.searchParams.get("tier")));
        case "/api/wand-lab/builds": return send(response, 200, listBuilds());
        case "/api/wand-lab/draft": return send(response, 200, draftStatus());
        case "/api/wand-lab/bestiary": return send(response, 200, bestiary());
      }
    }
    if (request.method === "POST") {
      if (["/api/portal-layout/preview", "/api/portal-layout/apply", "/api/portal-layout/revert"].includes(url.pathname)) {
        const result = portalCommand(url.pathname.split("/").at(-1), await body(request, 262144));
        if (!url.pathname.endsWith("/preview")) invalidateKnowledge();
        return send(response, 200, result);
      }
      if (["/api/material-progression/preview", "/api/material-progression/apply", "/api/material-progression/revert"].includes(url.pathname)) {
        const result = materialProgressionCommand(url.pathname.split("/").at(-1), await body(request, 32768));
        if (!url.pathname.endsWith("/preview")) invalidateKnowledge();
        return send(response, 200, result);
      }
      if (["/api/tool-behavior/preview", "/api/tool-behavior/apply", "/api/tool-behavior/revert"].includes(url.pathname)) {
        const result = toolBehaviorCommand(url.pathname.split("/").at(-1), await body(request, 32768));
        if (!url.pathname.endsWith("/preview")) invalidateKnowledge();
        return send(response, 200, result);
      }
      if (url.pathname === "/api/mod-model") {
        const { digest, size } = await body(request, 512);
        const { bytes, entries } = cachedMod(digest);
        return send(response, 200, inspectModel(bytes, (relative ?? "").slice(0, 400), size, entries));
      }
      if (url.pathname === "/api/start-model") {
        if (blocksPlaytest()) throw new Error("Wait for the active design run");
        const { asset, text } = await body(request, 32_768);
        game.startModelPreview(asset, text);
        return send(response, 200, game.status());
      }
      if (url.pathname === "/api/mod-evidence" || url.pathname === "/api/mod-class" || url.pathname === "/api/mod-texture") {
        const focus = (relative ?? "").slice(0, 2400);
        if (url.pathname === "/api/mod-evidence") {
          return send(response, 200, inspectMod(await rawBody(request, maxJarBytes), focus));
        }
        const archive = request.headers["content-type"] === "application/json"
          ? cachedMod((await body(request, 256)).digest)
          : { bytes: await rawBody(request, maxJarBytes) };
        return send(response, 200, url.pathname === "/api/mod-texture"
          ? inspectTexture(archive.bytes, focus, archive.entries)
          : await inspectClass(archive.bytes, focus, archive.entries));
      }
      if (url.pathname === "/api/navigation") return send(response, 200, writeNavigation(await body(request, 16_384)));
      if (url.pathname === "/api/parity/work") {
        return send(response, 202, startParityWork(await body(request, 4096)));
      }
      if (url.pathname === "/api/parity/work/stop") {
        const { id, run } = await body(request, 1024);
        return send(response, 200, stopParityWork(id, run));
      }
      if (url.pathname === "/api/parity/work/record") {
        const { id, revision } = await body(request, 1024);
        return send(response, 200, await recordParityWork(id, revision));
      }
      if (url.pathname === "/api/parity") {
        const { id, revision, status, evidence, note } = await body(request, 16_384);
        const change = {};
        if (status !== undefined) change.status = status;
        if (Array.isArray(evidence)) change.evidence = evidence.filter(entry => typeof entry === "string").slice(0, 16);
        if (typeof note === "string") change.note = note.slice(0, 600);
        try { return send(response, 200, writeStatus(id, change, revision, new Set(loadLedger().rows.map(row => row.id)))); }
        catch (error) { return send(response, 409, { error: error.message }); }
      }
      if (url.pathname === "/api/workbench") return send(response, 200, writeWorkbench(await body(request, 32_768)));
      if (url.pathname === "/api/sprite-draft") return send(response, 200, writeSpriteDraft(relative, await body(request)));
      if (url.pathname === "/api/workflows") return send(response, 200, await controlWorkflow(await body(request, workflowBytes), request.headers["x-axiom-control-forwarded"] === "1"));
      if (url.pathname === "/api/agent") return send(response, 202, startAgent(await body(request, agentBytes)));
      if (url.pathname === "/api/improvement") {
        return send(response, 202, startImprovement(await body(request, taskBytes * 4 + 16_384), game.status().logs));
      }
      if (url.pathname === "/api/improvement/cancel") {
        return send(response, 200, cancelImprovement((await body(request, 512)).id));
      }
      if (url.pathname === "/api/material") {
        const saved = saveMaterial(await body(request, 8192));
        invalidateKnowledge();
        return send(response, 200, saved);
      }
      if (url.pathname === "/api/material/suggest") {
        return send(response, 200, await suggestMaterial(await body(request, 8192), game.status()));
      }
      if (url.pathname === "/api/wand-lab/evaluate") return send(response, 200, await evaluateBoth(await body(request, 8192)));
      if (url.pathname === "/api/wand-lab/optimize") return send(response, 200, await optimizeWand(await body(request, 8192)));
      if (url.pathname === "/api/wand-lab/survey") return send(response, 200, await surveyWands(await body(request, 8192)));
      if (url.pathname === "/api/wand-lab/balance/suggest") return send(response, 200, await suggestBalance(await body(request, 8192), game.status()));
      if (url.pathname === "/api/wand-lab/draft") return send(response, 200, await editDraft(await body(request, 8192)));
      if (url.pathname === "/api/wand-lab/draft/reset") return send(response, 200, resetDraft());
      if (url.pathname === "/api/wand-lab/draft/apply") { const r = applyDraft(); invalidateKnowledge(); return send(response, 200, r); }
      if (url.pathname === "/api/wand-lab/suggest") return send(response, 200, await suggestWand(await body(request, 8192), game.status()));
      if (url.pathname === "/api/wand-lab/builds") return send(response, 200, saveBuild(await body(request, 8192)));
      if (url.pathname === "/api/wand-lab/builds/delete") return send(response, 200, deleteBuild(await body(request, 1024)));
      if (url.pathname === "/api/wand-lab/equip") {
        const build = await body(request, 8192);
        return send(response, 200, await engineRequest({ op: "wand", build, equip: true }));
      }
      if (url.pathname === "/api/craft/suggest") {
        return send(response, 200, await suggestRecipe(await body(request, 16_384), game.status()));
      }
      if (url.pathname === "/api/improvement/loop") {
        return send(response, 202, startImprovementLoop(await body(request, taskBytes + 16_384), game.status().logs));
      }
      if (url.pathname === "/api/improvement/check") {
        const { id, checkId } = await body(request, 1024);
        return send(response, 200, await checkAgentRegression(id, checkId));
      }
      if (url.pathname === "/api/improvement/next") {
        return send(response, 200, await proposeNext(await body(request, taskBytes + 16_384), game.status().logs,
          { receipts: decisionReceipts(), bundles: decisionBundles(), active: activeAgentPaths() }));
      }
      if (url.pathname === "/api/agent/context") {
        return send(response, 200, await collectContext(await body(request, taskBytes + 48_000), game.status().logs));
      }
      if (url.pathname === "/api/agent/cancel") return send(response, 200, cancelAgent((await body(request, 512)).id));
      if (url.pathname === "/api/agent/verify") return send(response, 200, recheckAgent((await body(request, 512)).id));
      if (url.pathname === "/api/agent/review") {
        const { id, outcome, note } = await body(request, 4096);
        return send(response, 200, reviewAgent(id, outcome, note));
      }
      if (url.pathname === "/api/notes") {
        const { title, systemBoard, purpose } = await body(request, 4096);
        return send(response, 200, createNote(title, systemBoard === true, purpose));
      }
      if (url.pathname === "/api/anchor-studio") return send(response, 200, await anchorStudio());
      if (url.pathname === "/api/food-workflow") {
        const command = await body(request, 16_384);
        const actions = { catalog: foodWorkflowCatalog, inspect: () => inspectFoodWorkflow(command.bundleId),
          preview: previewFoodWorkflow, apply: applyFoodWorkflow,
          "preview-withdraw": previewFoodWithdrawal, withdraw: withdrawFoodRecipe };
        if (!Object.hasOwn(actions, command?.op)) throw new Error("Unknown food workflow command");
        const result = await actions[command.op](command.request);
        if (["apply", "withdraw"].includes(command.op)) invalidateKnowledge();
        return send(response, 200, result);
      }
      if (url.pathname === "/api/save") {
        const saved = save(relative, await body(request));
        if (relative.endsWith(".rs")) invalidateKnowledge();
        return send(response, 200, saved);
      }
      if (url.pathname === "/api/save-agent-bundle") {
        const { changes, runId } = await body(request);
        if (!Array.isArray(changes) || !changes.length || changes.length > maxSources ||
            changes.some(change => !canAssist(change?.path) || typeof change.version !== "string" || typeof change.text !== "string") ||
            new Set(changes.map(change => change.path)).size !== changes.length) {
          throw new Error("Choose one to five distinct project drafts");
        }
        const saved = publishBundle(changes, runId);
        invalidateKnowledge();
        return send(response, 200, saved);
      }
      if (url.pathname === "/api/revert-agent-bundle") {
        const { id } = await body(request, 256);
        const restored = revertBundle(id);
        invalidateKnowledge();
        return send(response, 200, restored);
      }
      if (url.pathname === "/api/refine-design") {
        const { brief } = await body(request, 40_000);
        return send(response, 200, await refineDesign(brief, game.status()));
      }
      if (url.pathname === "/api/render-tool-art") return send(response, 200, await renderToolArt());
      if (url.pathname === "/api/render-saved-tool-art") return send(response, 200, await renderSavedToolArt());
      if (["/api/build", "/api/start", "/api/start-recipe", "/api/start-spark"].includes(url.pathname) && blocksPlaytest()) {
        throw new Error("Wait for the local design review to finish.");
      }
      if (url.pathname === "/api/engine") return send(response, 200, await engineRequest(await body(request, 32000)));
      if (url.pathname === "/api/engine/close") return send(response, 200, await game.closeExperiment());
      if (url.pathname === "/api/engine/preset") return send(response, 200, await enginePreset(await body(request, 2048)));
      if (url.pathname === "/api/engine/sample") return send(response, 200, await engineSample(await body(request, 2048)));
      if (url.pathname === "/api/engine/start") {
        if (blocksPlaytest()) throw new Error("Wait for the local design review to finish.");
        return send(response, 200, startEngineSession(await body(request, 2048)));
      }
      if (url.pathname === "/api/build") game.rebuild();
      else if (url.pathname === "/api/start") game.start(url.searchParams.has("existing"));
      else if (url.pathname === "/api/start-recipe") {
        const { text, rebuild, recipe } = await body(request);
        if (typeof text !== "string") throw new Error("Choose a recipe draft to playtest");
        if (blocksPlaytest()) throw new Error("Wait for the local design review to finish.");
        if (rebuild === true) game.buildRecipePreview(text, recipe);
        else game.start(false, text);
      }
      else if (url.pathname === "/api/start-spark") {
        const draft = await body(request, 1024);
        game.start(false, undefined, sparkPreview(read(spellPath), draft));
      }
      else if (url.pathname === "/api/command") return send(response, 200, { reply: await game.command((await body(request)).line) });
      else return send(response, 404, { error: "Action not found" });
      return send(response, 200, game.status());
    }
    return send(response, 404, { error: "Action not found" });
  } catch (error) {
    if (!response.headersSent) send(response, error.status ?? 400, { error: error.message });
  }
});

if (process.argv.includes("--check-ui-imports")) {
  const result = checkUiImports();
  console.log(JSON.stringify(result));
  if (!result.ok) process.exitCode = 1;
} else {
  const previous = worker.agentLane();
  if (previous && !previous.alive) worker.recoverAgentLane();
  const lease = worker.acquireAgentLane("AXIOM Dev server");
  process.on("exit", () => {
    worker.releaseAgentLane(lease);
    rmSync(workerSession, { force: true });
  });
  recoverWorkflowStore();
  server.on("error", error => {
    console.error(error.code === "EADDRINUSE" ? "AXIOM Dev address is already in use. Open the shortcut to reconnect; no second worker was started." : error.message);
    process.exit(1);
  });
  server.listen({ port: appPort, host: "127.0.0.1", exclusive: true }, () => {
  origin = "http://127.0.0.1:" + server.address().port;
  const url = origin + "/#" + token;
  ensureDirectory(state);
  const session = JSON.stringify({protocol:1,origin,token,pid:process.pid,started:new Date().toISOString()});
  writeFileSync(workerSession,session,{mode:0o600});
  const temporary = path.join(state,"engine-session-" + process.pid + ".pending");
  writeFileSync(temporary,session,{mode:0o600});
  renameSync(temporary,path.join(state,"engine-session.json"));
  console.log(url);
  currentRevision = appRevision();
  loadedBackend = backendRevision();
  setInterval(() => {
    try {
      const revision = appRevision();
      if (revision !== currentRevision) {
        currentRevision = revision; changedAt = Date.now();
        updating = backendRevision() !== loadedBackend;
      }
      if (!updating || stopping || Date.now() - changedAt < 1500) return;
      const { running, building } = game.status();
      if (running || building || [agentLane(), parityLane()].some(owner => owner?.alive)) return;
      stopping = true;
      server.close(() => process.exit(0));
      // closeIdleConnections() left the page's event stream and the swarm proxy's keep-alive open, so close()
      // never finished: 2026-11-21 the server sat half-stopped (no listener, scheduler off) and Worker.ps1 never
      // restarted it. Drop every connection, and exit regardless after 5 s.
      server.closeAllConnections();
      setTimeout(() => process.exit(0), 5000);
    } catch (error) { console.error("Automatic update: " + error.message); }
  }, 1000).unref();
  startWorkflowScheduler(() => game.status().logs, () => !stopping && !updating);
  setImmediate(() => {
    if (!stopping) startParityRefreshScheduler({ shouldStart: () => !stopping && !updating
      && !game.status().building && !game.status().running });
  });
  if (process.argv.includes("--open") && process.platform === "win32") {
    import("node:child_process").then(({ spawn }) => {
      const browser = spawn("explorer.exe", [url], { detached: true, stdio: "ignore" });
      browser.on("error", error => console.error(error.message));
      browser.unref();
    });
  }
  });
}

