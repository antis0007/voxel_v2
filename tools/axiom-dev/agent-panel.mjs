import { el, button, findEntry } from "./inspector.mjs";
import { mountWorkflow } from "./workflow-panel.mjs";
import { copyText } from "./context-menu.mjs";
import { address, resolve, rustTunables } from "./targets.mjs";
import { canAssist, maxSources, maxTaskLength, assertTask, maxExcerpt, maxPackReference, cleanProposal, prepareProposal, proposalSchema } from "./agent-contract.mjs";
import { fabricationPath, fabricationRows } from "./fabrication-data.mjs";
import { openTextureStudy } from "./texture-study.mjs";
import { cells, isSheet, readSheet, rgba } from "./sprite-sheet.mjs";
import { canvasPixels } from "./pixel-grid.mjs";
import { openModelStudy, openPackIntake } from "./mod-pack.mjs";

const name = path => path.split("/").at(-1);
const muted = value => el("p", { className: "muted" }, value);
const option = (value, text) => el("option", { value }, text);
const labeled = (text, element) => el("label", {}, text, element);
const promptSize = bytes => bytes ? (bytes / 1024).toFixed(1) + " KiB of prompt text" : "";
const campaigns = [
  ["Crafting", "Trace how the player obtains the ingredients, uses the station, and finds a use for the output.", ["assets/recipes.ron", "assets/progression.ron"]],
  ["Machines", "Trace inputs, power, outputs, and one measured production cycle.", ["assets/machines.ron", "crates/app/src/world/factory/recipes.rs"]],
  ["Magic", "Compare costs, timing, counters, and native spell execution.", ["assets/spells.ron", "tools/axiom-dev/FORGE_SPARK.md"]],
  ["Creatures", "Trace the habitat, attack and recovery windows, and cues visible to players.", ["assets/bestiary.ron", "assets/spawns.ron"]],
  ["Animation", "Preserve authored stylization and compare motion under matched camera and lighting.", ["assets/runtime/viewmodels/presentation.ron", "crates/app/src/world/adventure/wildlife/resident_model.rs"]],
  ["Materials", "Trace extraction, transformations, uses, and appearance in a native scene.", ["assets/materials.ron", "assets/reactions.ron"]],
  ["Civilization", "Trace geography, resident work, supplies, and the outcome across time and reload.", ["crates/worldgen/src/civilization.rs", "crates/app/src/world/travel/civilization.rs"]],
  ["Dimensions", "Trace entry, a distinct gameplay decision, and a playable return.", ["assets/star_systems.ron", "crates/worldgen/src/nether.rs"]],
];
const improvementLanes = [
  ["engine", "Engine", "crates/app/src/render/coverage.rs",
    "Audit ResidentCoverage for a concrete incorrect coverage result, especially across voxel scales and window edges."],
  ["performance", "Optimization", "crates/app/src/render/upload_staging.rs",
    "Inspect UploadStaging::reserve and release_after_submit for repeat work or contention during terrain upload bursts."],
  ["art", "Art refinement", "assets/icons/items16.txt",
    "Suggest a small contrast refinement for bucket_empty (sprite 0). Keep its outline, silhouette, and pixel-art style."],
  ["tooling", "Developer tools", "tools/axiom-dev/agent-context.mjs",
    "Inspect context retrieval for stale evidence, missing owners or inaccurate provenance."],
];

function spriteComparisons(value, snapshot) {
  if (snapshot.some(source => source.anchor && source.doc.version !== source.anchor.version)) {
    throw new Error("The saved source changed. Run again on the current asset.");
  }
  const changes = prepareProposal(value, snapshot).filter(({ doc }) => isSheet(doc.path));
  const previews = [];
  for (const { doc, text } of changes) {
    const before = readSheet(doc.text, doc.path);
    const after = readSheet(text, doc.path);
    const originals = new Map(before.entries.map(entry => [entry.id, entry]));
    const pixels = entry => {
      const colors = new Map(entry.palette.map(swatch => [swatch.key.charCodeAt(0), rgba(swatch.rgb)]));
      return cells(entry).map(value => colors.get(value) ?? 0);
    };
    for (const entry of after.entries) {
      const prior = originals.get(entry.id);
      if (!prior) continue;
      const oldPixels = pixels(prior), newPixels = pixels(entry);
      if (oldPixels.every((value, index) => value === newPixels[index])) continue;
      const preview = (label, data) => {
        const canvas = el("canvas", { className: "agent-sprite", role: "img", "aria-label": entry.name + " " + label });
        canvasPixels(canvas, data, 16, 16);
        return el("figure", {}, canvas, el("figcaption", {}, label));
      };
      previews.push(el("details", { className: "agent-edit", open: true }, el("summary", {}, entry.name),
        el("div", { className: "agent-diff" }, preview("Before", oldPixels), preview("Candidate", newPixels))));
      if (previews.length === 4) return previews;
    }
  }
  return previews;
}

function evidenceView(evidence, attempts = []) {
  if (!evidence && !attempts.length) return null;
  return el("details", { className: "agent-edit" },
    el("summary", {}, "Candidate evidence · " + (evidence?.status ?? "No candidate")),
    el("ul", { className: "agent-checks" }, (evidence?.files ?? []).map(file =>
      el("li", { title: file.after ? "SHA-256 " + file.after : "" },
        name(file.path) + " · " + file.name + " · " + file.status + (file.detail ? " · " + file.detail : "")))),
    el("ul", { className: "agent-checks" }, attempts.map(row => el("li", {},
      "Pass " + row.pass + " · " + row.status + " · " + (row.elapsedMs / 1000).toFixed(1) + " s"
        + (row.input !== undefined ? " · " + row.input + " in / " + row.output + " out" : "")
        + (row.diagnostic ? " · " + row.diagnostic : "")))));
}

export function mountAgent(host, ctx) {
  const submittedTaskKey = "axiom.ai.submitted-task";
  let submittedTask = "";
  try { submittedTask = sessionStorage.getItem(submittedTaskKey) ?? ""; } catch { /* Private window. */ }
  let attached = [], capture = [], proposal, active, busy = false, polling;
  let modReference = "", toolkitReference = "", stageOnComplete = false;
  let modTexture, textureReference = "", modelReference = "";
  let knowledge;
  const sourceList = el("div", { className: "agent-sources" });
  const contextPanel = el("div", { className: "agent-sources" });
  const status = el("span", { role: "status" }, "Idle");
  const history = button("Run history", async () => {
    try {
      const rows = (await ctx.api("/api/agent/receipts")).reverse();
      ctx.dialog("Run history", rows.length ? rows.map(row => el("details", { className: "agent-edit" },
        el("summary", {}, new Date(row.at).toLocaleString() + " · " + (row.phase ?? row.outcome) + " · " + (row.mode ?? "Review")),
        row.provider ? muted([row.provider, row.model, row.edits + " edits", (row.usage?.input ?? 0) + " input tokens"].filter(Boolean).join(" · ")) : null,
        row.promptBytes ? muted(promptSize(row.promptBytes)) : null,
        row.outcomeSummary ? el("p", {}, row.outcomeSummary) : null,
        row.note ? muted(row.note) : null, evidenceView(row.evidence, row.attempts),
        row.phase === "Ready" && row.sources?.length ? button("Continue with feedback", event => {
          event.currentTarget.closest("dialog")?.close();
          prepareFollowUp(row.sources, row.outcomeSummary);
        }) : null,
        row.sources?.length ? el("ul", { className: "agent-checks" }, row.sources.map(source => el("li", {}, source.path))) : null,
      )) : [muted("No recorded runs.")], "Close", () => true);
    } catch (error) { ctx.note(error.message, true); }
  });
  const result = el("section", { className: "craft-window agent-result" }, el("header", { className: "craft-titlebar" }, el("h2", {}, "Proposal")));
  const resultBody = el("div", { className: "agent-result-body" }, muted("No Proposal"));
  result.append(resultBody);
  const request = el("textarea", { rows: 4, placeholder: "e.g. Reduce this recipe's steel cost while keeping its shape.",
    "aria-label": "Task" });
  const provider = el("select", {}, option("codex", "Codex"), option("ollama", "Ollama"), option("lmstudio", "LM Studio"));
  const model = el("input", { type: "text", placeholder: "Codex default", maxLength: 100 });
  provider.onchange = () => { model.placeholder = provider.value === "codex" ? "Codex default" : "Local model name"; };
  const mode = el("select", {}, option("refine", "Prototype"), option("systems", "Systems Review"), option("interface", "UI Review"),
    option("port", "Mod Port"), option("pack", "Modpack Synthesis"), option("toolkit", "Toolkit Improvement"),
    option("audit", "Flaw Audit"), option("repair", "Audit & Repair"));
  const repeat = option("4", "4 · Audit candidate and repair again");
  const passes = el("select", {}, [1, 2, 3].map(n => option(String(n), String(n))), repeat);
  mode.onchange = () => {
    if (mode.value === "repair") passes.value = "4";
    renderSources();
  };
  const run = button("Run proposal", runProposal, "Send the attached excerpts to the selected provider");
  const createMission = button("Create mission", () => openMissionComposer(), "Plan and queue work from this task", "primary");
  const gather = button("Gather Context", gatherContext, "Find related source, symbol references, and current diagnostics");
  const importMod = button("Import Mod", openMod, "Start from a feature description or local JAR");
  const pack = button("Import Modpack", () => openPackIntake(ctx, preparePack), "Scan up to 120 mods and compare progression");
  const textures = button("Texture Study", () => openTextureStudy(ctx, modTexture, evidence => {
    textureReference = evidence;
    artPreview.textContent = [textureReference, modelReference].filter(Boolean).join(" · ");
  }), "Generate original sprite art from a local reference");
  const improve = button("Improve Toolkit", prepareToolkit, "Use recent run receipts to propose one tooling improvement");
  const stage = el("input", { type: "checkbox" });
  const evidenceOnRun = el("input", { type: "checkbox" });
  const referencePreview = el("div");
  const artPreview = muted("");
  const stop = button("Stop", async () => {
    if (!active) return;
    try { await ctx.api("/api/agent/cancel", undefined, { id: active }); }
    catch (error) { ctx.note(error.message, true); }
  });
  stop.disabled = true;
  const add = button("+ Source", () => {
    const entries = [...ctx.paths()].filter(canAssist).map(path => ({ key: path, label: name(path), detail: path }));
    findEntry("Attach Source", entries, null, path => attach({ path, label: name(path) }));
  });
  const copy = button("Copy Task", async () => {
    try {
      const sources = await snapshot();
      const context = contextIsCurrent()
        ? { evidence: knowledge.evidence, diagnostics: knowledge.diagnostics, warnings: knowledge.warnings } : undefined;
      await copyText(JSON.stringify({ task: request.value, mode: mode.value,
        reference: reference(), sources: publicSources(sources), context, schema: proposalSchema }, null, 2));
      ctx.note("Task and evidence copied");
    } catch (error) { ctx.note(error.message, true); }
  }, "Copy the task, excerpts, and proposal schema");
  const importButton = button("Import Proposal", () => {
    const json = el("textarea", { rows: 9, required: true, "aria-label": "Proposal JSON" });
    ctx.dialog("Import Proposal", [json], "Preview", async () => {
      const sources = await snapshot();
      const value = cleanProposal(JSON.parse(json.value), sources);
      capture = sources;
      show(value);
      status.textContent = "Imported";
      return true;
    });
  });
  const campaign = button("Campaigns", () => {
    const pick = el("select", {}, campaigns.map(([label], index) => option(String(index), label)));
    const focus = muted(campaigns[0][1]);
    pick.onchange = () => { focus.textContent = campaigns[Number(pick.value)][1]; };
    ctx.dialog("Campaign brief", [labeled("Game system", pick), focus,
      muted("Load a new brief and scope. You can edit both before running.")], "Load brief", () => {
      const [label, criteria, paths] = campaigns[Number(pick.value)];
      request.value = "Improve one " + label.toLowerCase() + " workflow. " + criteria
        + " Explain the player benefit, affected dependencies, tradeoffs, and evidence still required.";
      mode.value = "systems";
      passes.value = "3";
      stage.checked = false;
      attached = [];
      const available = new Set(ctx.paths());
      paths.filter(path => available.has(path)).forEach(path => attach({ path, label: name(path) }));
      renderSources();
      return true;
    });
  });
  let queueTimer, queueRunning = false;
  const queueBody = el("div", { className: "agent-result-body" });
  const queueStart = button("Set Up Improvement Run", () => {
    const fields = improvementLanes.map(([lane, label, path, task]) => ({
      lane, label,
      selected: el("input", { type: "checkbox", checked: lane !== "tooling" }),
      path: el("input", { type: "text", value: path, maxLength: 240, "aria-label": label + " source" }),
      task: el("textarea", { rows: 2, value: task, "aria-label": label + " brief" }),
    }));
    const selectedProvider = provider.value;
    const selectedModel = model.value.trim();
    ctx.dialog("Set up improvement run", [
      muted("Using " + selectedProvider + (selectedModel ? " · " + selectedModel : "") +
        (selectedProvider === "codex" ? ". Up to two workers run at once." : ". One worker runs at a time to limit local load.") +
        " Each task needs its own source file. Review each result before staging."),
      ...fields.map(field => el("div", { className: "stack" }, labeled(field.label, field.selected),
        labeled("Source", field.path), labeled("Brief", field.task))),
    ], "Start Run", async () => {
      const tasks = fields.filter(field => field.selected.checked)
        .map(field => ({ lane: field.lane, path: field.path.value.trim(), task: field.task.value.trim() }));
      renderQueue(await ctx.api("/api/improvement", undefined, { provider: selectedProvider, model: selectedModel, tasks }));
      return true;
    });
  });
  const nextMove = button("Propose Next Move", async () => {
    const task = request.value.trim();
    if (!task) { request.focus(); return ctx.note("Describe the outcome to investigate first.", true); }
    const lane = el("select", {}, improvementLanes.map(([value, label]) => option(value, label)));
    let planned;
    ctx.dialog("Choose the next investigation", [labeled("Lane", lane), muted(task),
      muted("Checks current sources, recorded outcomes and Forge ownership. Proposals and tests remain separate from adoption.")], "Find Next Move", async () => {
      planned = await ctx.api("/api/improvement/next", undefined, { task, lane: lane.value,
        targets: contextTargets(), modDigests: modTexture ? [modTexture.digest] : [] });
      // Open after the current dialog closes.
      setTimeout(() => showNextMove(planned), 0);
      return true;
    });
  });
  function showNextMove(result) {
    const decision = result.decision;
    if (!decision) {
      ctx.dialog("Next evidence needed", [muted(result.next),
        ...(result.skipped ?? []).map(row => muted(row.path + " — " + row.reason))], "Close", () => true);
      return;
    }
    ctx.dialog("Proposed next move", [el("strong", {}, decision.path), muted(decision.hypothesis),
      el("ul", {}, decision.acceptance.map(text => el("li", {}, text))),
      muted("Dependencies: " + decision.dependencies.join(", ")),
      ...decision.referenceFacts.map(row => muted("Reference SHA-256 " + row.digest + " — " + row.reuse)),
      muted(result.limits), muted("Verification pending. The result will wait for review.")], "Run Investigation", async () => {
      renderQueue(await ctx.api("/api/improvement", undefined, { provider: provider.value, model: model.value.trim(),
        tasks: [{ decisionId: decision.id }] }));
      return true;
    });
  }
  const boundedLoop = button("Investigate & verify", () => {
    const task = request.value.trim();
    if (!task) { request.focus(); return ctx.note("Describe the outcome to investigate first.", true); }
    const lane = el("select", {}, improvementLanes.map(([value, label]) => option(value, label)));
    const budget = el("select", {}, [1, 2, 3].map(value => option(String(value), String(value))));
    budget.value = "2";
    ctx.dialog("Investigate and verify", [muted(task), labeled("Lane", lane), labeled("Maximum investigations", budget),
      muted("Finds a source to investigate, skips work already tried on the same evidence, and calls a registered verification check when available. Stops when a candidate is ready to review.")],
    "Start investigation", async () => {
      renderQueue(await ctx.api("/api/improvement/loop", undefined, { task, lane: lane.value,
        maxSteps: Number(budget.value), provider: provider.value, model: model.value.trim(),
        targets: contextTargets(), modDigests: modTexture ? [modTexture.digest] : [] }));
      return true;
    });
  });
  let queueId;
  const queueStop = button("Stop Queue", async () => {
    queueStop.disabled = true;
    try { renderQueue(await ctx.api("/api/improvement/cancel", undefined, { id: queueId })); }
    catch (error) { queueStop.disabled = false; ctx.note(error.message, true); }
  });
  queueStop.hidden = true;
  const queue = el("section", { className: "craft-window" },
    el("header", { className: "craft-titlebar" }, el("h2", {}, "Improvement Queue"), nextMove, boundedLoop, queueStart, queueStop), queueBody);

  function renderQueue(state) {
    clearTimeout(queueTimer);
    const running = state.phase === "Running";
    const workers = state.rows.filter(row => row.status === "Running" || row.status === "Gathering context").length;
    queueRunning = running;
    queueId = state.id;
    queueStop.hidden = !running;
    queueStop.disabled = Boolean(state.stopping);
    renderSources();
    queueBody.replaceChildren(...(state.error ? [muted(state.error)] : []),
      ...(state.next?.next ? [muted(state.next.next)] : []),
      el("p", { className: "muted", role: "status" }, state.phase === "Idle"
        ? "Pick the areas you want AI workers to investigate."
        : state.stopping ? "Stopping workers…"
        : state.planning && running ? "Finding the next source to investigate…"
        : running && state.concurrency ? workers + " of " + state.concurrency + " workers active" : state.phase),
      ...state.rows.map(row => {
        const label = improvementLanes.find(([lane]) => lane === row.lane)?.[1] ?? row.lane;
        return el("div", { className: "draft-row" }, el("strong", {}, label + " · " + name(row.path)),
          muted([row.status, row.status === "Running" && row.step, row.evidence, row.verification, row.regression].filter(Boolean).join(" · ")),
          row.usage?.input || row.usage?.output ? muted(row.usage.input.toLocaleString() + " input tokens · " +
            row.usage.output.toLocaleString() + " output tokens") : row.promptBytes ? muted(promptSize(row.promptBytes)) : null,
          row.error ? muted(row.error) : null,
          row.runId && row.status !== "Running" ? button(row.edits ? "Review Candidate" : "View Result",
            () => openQueuedResult(row.runId)) : null);
      }));
    if (running) queueTimer = setTimeout(refreshQueue, 1500);
  }

  async function refreshQueue() {
    try { renderQueue(await ctx.api("/api/improvement")); }
    catch (error) { queueBody.replaceChildren(muted(error.message), button("Retry", refreshQueue)); }
  }

  async function openQueuedResult(id) {
    if (busy) return ctx.note("Wait for the current AI run.");
    try {
      const state = await ctx.api("/api/agent?run=" + encodeURIComponent(id));
      if (state.id !== id) throw new Error("This queue result is no longer available.");
      const sources = await loadRunSources(state.autoSources ?? []);
      if (busy) return;
      capture = sources;
      show(state.proposal ?? { summary: state.error ?? state.phase, checks: [], edits: [] },
        false, state.proposal ? id : undefined, state.evidence, state.attempts, state.finding, state.checkId, state.regression);
      result.scrollIntoView({ block: "nearest" });
    } catch (error) { ctx.note(error.message, true); }
  }

  const loadRunSources = sources => Promise.all(sources.map(async source => {
    const doc = await ctx.load(source.path);
    return { ...source, doc, start: source.anchor.start, original: doc.original };
  }));

  const parityTaskLink = el("a", { hidden: true, target: "_blank", rel: "noopener" });
  let currentParityId;
  const columns = el("div", { className: "agent-scroll" });
  const composer = el("div", { id: "agent-composer", className: "agent-columns" });
  const taskView = button("Task", () => showView(false));
  const missions = button("Missions", () => { showView(true); missionPanel.open(); });
  taskView.setAttribute("aria-controls", composer.id);
  missions.setAttribute("aria-controls", "agent-missions");
  host.append(el("header", { className: "agent-toolbar" }, el("h1", {}, "AI Workbench"),
    el("nav", { className: "agent-views", "aria-label": "AI workbench views" }, taskView, missions),
    status, parityTaskLink, history, stop), columns);
  columns.append(composer);
  composer.append(
    el("div", { className: "agent-setup" },
      el("section", { className: "craft-window" }, el("header", { className: "craft-titlebar" }, el("h2", {}, "Describe the outcome")),
        el("div", { className: "agent-fields" }, request,
          el("div", { className: "agent-actions" }, createMission, run),
          el("details", { className: "agent-edit agent-more" }, el("summary", {}, "Run settings and tools"),
            el("div", { className: "agent-options" }, labeled("Mode", mode), labeled("Passes", passes),
              labeled("Provider", provider), labeled("Model", model)),
            labeled("Auto-stage checked edits as drafts", stage),
            el("div", { className: "agent-actions" }, campaign, copy, importMod, pack, textures, improve, importButton)),
          referencePreview, artPreview)),
      el("details", { className: "craft-window agent-context", open: true }, el("summary", {}, "Project context"),
        el("div", { className: "agent-context-actions agent-actions" }, add, gather),
        el("div", { className: "agent-evidence-option" }, labeled("Gather project and live evidence on run", evidenceOnRun)),
        sourceList, contextPanel),
      el("details", { className: "craft-window agent-context" }, el("summary", {}, "Improvement queue"), queue)),
    result);
  request.addEventListener("input", renderContext);
  const missionPanel = mountWorkflow(columns, ctx, () => ({ task: request.value.trim(), paths: attached.map(row => row.path), parityId: currentParityId,
    provider: provider.value, model: model.value.trim() }));
  function showView(showMissions) {
    if (composer.hidden !== showMissions) columns.scrollTop = 0;
    composer.hidden = showMissions;
    missionPanel.section.hidden = !showMissions;
    taskView.setAttribute("aria-pressed", String(!showMissions));
    missions.setAttribute("aria-pressed", String(showMissions));
  }
  function openMissionComposer(source) {
    showView(true);
    void missionPanel.create(source);
  }
  showView(false);

  const contextTargets = () => attached.map(({ path, line, parts, name }) => ({ path, line, parts, name }));
  const contextIsCurrent = () => knowledge?.task === request.value.trim()
    && knowledge.scopeKey === JSON.stringify(contextTargets())
    && JSON.stringify(knowledge.modReferences?.map(row => row.digest) ?? []) === JSON.stringify(modTexture ? [modTexture.digest] : []);
  let contextQuery;
  async function collect(task, symbols) {
    const targets = contextTargets();
    const packet = await ctx.api("/api/agent/context", undefined, { task, symbols, targets,
      modDigests: modTexture ? [modTexture.digest] : [], runtime: true });
    knowledge = { ...packet, scopeKey: JSON.stringify(targets) };
    contextQuery = { task, symbols };
  }

  function gatherContext() {
    const task = request.value.trim();
    if (!task) { request.focus(); return ctx.note("Describe the subsystem or suspected flaw first", true); }
    const names = el("input", { type: "text", maxlength: 800, placeholder: "e.g. distribute_power, battery_rate",
      value: contextQuery?.task === task ? contextQuery.symbols.join(", ") : "" });
    const audit = el("input", { type: "checkbox", checked: mode.value === "audit" });
    ctx.dialog("Gather project context", [
      labeled("Optional Rust symbols", names), labeled("Use read-only Flaw Audit mode", audit),
      muted("Searches the local project and relevant playtest diagnostics. The evidence is shown before you run the AI."),
    ], "Gather Context", async () => {
      const symbols = names.value.split(/[,\s]+/).filter(Boolean);
      await collect(task, symbols);
      if (audit.checked) mode.value = "audit";
      renderSources();
      return true;
    });
  }

  function renderContext() {
    const current = contextIsCurrent();
    contextPanel.replaceChildren();
    if (!knowledge) return;
    if (!current) {
      contextPanel.append(muted("Task or attached sources changed. Gather fresh context."));
      return;
    }
    const clear = button("Remove Context", () => { knowledge = undefined; renderSources(); });
    clear.disabled = busy;
    contextPanel.append(el("div", { className: "agent-actions" },
      el("strong", {}, knowledge.evidence.length + " read-only excerpts"), clear),
    muted(knowledge.sampledFiles + " files sampled · " + (knowledge.crossReferences ? "Symbol search completed" : "Path and excerpt matching")));
    if (attached.length && ["refine", "systems", "interface"].includes(mode.value)) {
      contextPanel.append(muted("The model receives your attachments and a limited selection of this gathered evidence."));
    }
    if (knowledge.symbols.length) contextPanel.append(muted("Symbols searched · " + knowledge.symbols.join(", ")));
    if (knowledge.runtime) contextPanel.append(el("details", { className: "agent-excerpt" },
      el("summary", {}, knowledge.runtime.unavailable ? "Native engine unavailable" : "Native engine · " + knowledge.runtime.capturedAt),
      el("pre", {}, JSON.stringify(knowledge.runtime, null, 2))));
    for (const warning of knowledge.warnings ?? []) contextPanel.append(muted(warning));
    for (const reference of knowledge.modReferences ?? []) contextPanel.append(el("details", { className: "agent-excerpt" },
      el("summary", {}, "Mod reference — " + reference.digest.slice(0, 12)), muted(reference.reuse),
      el("pre", {}, reference.report)));
    for (const row of knowledge.evidence) {
      const addSource = button("Attach as Editable", () => attach({ path: row.path, line: row.line }));
      addSource.disabled = busy || attached.length >= maxSources || mode.value === "audit";
      contextPanel.append(el("details", { className: "agent-excerpt" },
        el("summary", {}, row.kind + " · " + name(row.path) + ":" + row.line),
        row.via ? muted("Pinned in " + row.via) : null,
        el("pre", {}, row.excerpt),
        el("div", { className: "agent-actions" },
          button("Open Source", () => ctx.open(row.path, undefined, row.line)), addSource)));
    }
    if (knowledge.diagnostics.length) contextPanel.append(el("details", { className: "agent-excerpt" },
      el("summary", {}, "Recent related diagnostics"),
      el("pre", {}, knowledge.diagnostics.map(row => row.message).join("\n"))));
  }

  function reference() {
    if (mode.value === "port" || mode.value === "pack") {
      const art = [textureReference && "Texture study: " + textureReference, modelReference && "Model study: " + modelReference]
        .filter(Boolean).map(text => text + "\n\n").join("");
      return (art + modReference).slice(0, mode.value === "pack" ? maxPackReference : 18000);
    }
    return mode.value === "toolkit" ? toolkitReference : "";
  }

  async function preparePack(evidence, auto) {
    attached = [];
    modTexture = undefined;
    textureReference = modelReference = "";
    artPreview.textContent = "";
    modReference = evidence;
    mode.value = "pack";
    passes.value = "3";
    request.value = "Synthesize this entire pack into an Axiom-native progression. Compare mechanics and implementation leads, then pick, merge, or drop overlapping features using Axiom lore and existing systems. Decide which machine routes earn a place. Normalize recipe costs per output, including catalysts, by-products, power, throughput and unlocks. Flag exploits, incomparable routes, missing evidence and intentional deviations. Propose only the first self-contained playable slice in the attached files. Give the rest a dependency-ordered delivery plan.";
    const paths = new Set(ctx.paths());
    for (const path of ["docs/design/WORLD_LORE.md", "docs/plans/MOD_PARITY_DELIVERY.md", "assets/fabrication.ron",
      "crates/app/src/world/factory/recipes.rs", "assets/progression.ron"]) {
      if (paths.has(path)) attach({ path });
    }
    referencePreview.replaceChildren(el("details", { className: "agent-excerpt" },
      el("summary", {}, "Modpack evidence"), el("pre", {}, evidence)));
    renderSources();
    if (auto) {
      stage.checked = true;
      await runProposal();
    }
  }

  async function prepareToolkit() {
    if (busy) return;
    try {
      const receipts = await ctx.api("/api/agent/receipts");
      if (!receipts.length) return ctx.note("Run and review a workbench proposal first", true);
      ctx.dialog("Toolkit improvement", [
        muted("Use recent local run receipts and human feedback. Replace the current task and its source selection?"),
        el("pre", {}, JSON.stringify(receipts.slice(-4), null, 2)),
      ], "Prepare review", () => {
        attached = [];
        toolkitReference = JSON.stringify(receipts).slice(-18000);
        request.value = "Review recent workbench outcomes and propose one measurable improvement to mod intake, source routing, or candidate validation.";
        mode.value = "toolkit";
        passes.value = "2";
        for (const path of ["tools/axiom-dev/agent-contract.mjs", "tools/axiom-dev/agent-runner.mjs",
          "tools/axiom-dev/agent-panel.mjs", "tools/axiom-dev/mod-evidence.mjs", "docs/plans/AXIOM_DEV_TOOLKIT.md"]) {
          if (ctx.paths().includes(path)) attach({ path });
        }
        renderSources();
        return true;
      });
    } catch (error) { ctx.note(error.message, true); }
  }

  function openMod() {
    const brief = el("textarea", { rows: 5, placeholder: "Describe the feature and the player loop you want in Axiom." });
    const jar = el("input", { type: "file", accept: ".jar" });
    ctx.dialog("Minecraft mod intake", [
      labeled("Feature description", brief), labeled("Optional local mod JAR", jar),
      muted("JAR inspection runs locally. Run sends the displayed evidence and selected Axiom excerpts to your chosen provider."),
    ], "Prepare mod port", async () => {
      const task = brief.value.trim();
      const file = jar.files[0];
      if (!task && !file) throw new Error("Describe the feature or select a JAR");
      if (file && (!/\.jar$/i.test(file.name) || file.size > 48 * 1024 * 1024)) {
        throw new Error("Choose a .jar file up to 48 MiB");
      }
      const evidence = file ? await ctx.api("/api/mod-evidence", task, file) : null;
      modReference = evidence?.report ?? "";
      modTexture = file ? { digest: evidence.digest, paths: evidence.textures, models: evidence.models } : undefined;
      textureReference = modelReference = "";
      artPreview.textContent = "";
      request.value = task || "Find a distinctive player-facing feature in this mod and build the smallest complete native Axiom version.";
      mode.value = "port";
      if (!attached.length) {
        const methods = evidence?.behavior?.methods ?? [];
        const leads = methods.map(row => [row.owner, row.method, ...row.roles, ...row.data].join(" ")).join("\n");
        await suggestOwners(task, leads || modReference);
      }
      passes.value = attached.length ? "3" : "1";
      if (evidence) {
        const content = el("pre", {}, evidence.report);
        const coverage = evidence.coverage;
        referencePreview.replaceChildren(
          ...coverage.declared.map(row => muted([row.loader, ...row.modIds, ...row.minecraft].join(" · "))),
          ...coverage.boundaries.map(boundary => muted(boundary)),
          el("details", { className: "agent-excerpt" }, el("summary", {}, "JAR evidence · " + file.name), content),
          evidence.models.length ? button("Model Study", () => {
            if (busy) return;
            openModelStudy(ctx, modTexture, info => {
              modelReference = info;
              artPreview.textContent = [textureReference, modelReference].filter(Boolean).join(" · ");
            });
          }) : null,
          button("Inspect Class", () => {
            if (busy) return;
            const name = el("input", { type: "text", placeholder: "Class path or bundled.jar!class/path", maxlength: 400 });
            ctx.dialog("Inspect a JAR class", [labeled("Class path", name)], "Inspect", async () => {
              const focused = await ctx.api("/api/mod-class", name.value.trim(), { digest: evidence.digest });
              modReference = focused.report + "\n\n" + evidence.report.slice(0, 18000 - focused.report.length - 2);
              content.textContent = modReference;
              return true;
            });
          }),
        );
      } else referencePreview.replaceChildren(muted("Description only"));
      renderSources();
      return true;
    });
  }

  async function suggestOwners(task, reference) {
    const routes = [
      [/fabricat|machine|energy|power|generator|battery|automat/, ["assets/fabrication.ron", "crates/app/src/world/factory/recipes.rs", "crates/app/src/world/factory/power.rs"]],
      [/spell|wand|mana|beacon|turret/, ["assets/spells.ron", "crates/app/src/world/habitats.rs", "crates/app/src/world/adventure.rs"]],
      [/crop|farm|seed|flower/, ["assets/crops.ron", "crates/app/src/world/farming/flowers.rs"]],
      [/biome|worldgen|ore|deposit/, ["assets/materials.ron", "crates/worldgen/src/lib.rs"]],
      [/creature|monster|mob|animal/, ["assets/bestiary.ron", "assets/spawns.ron"]],
      [/craft|recipe|food|item/, ["assets/recipes.ron", "assets/foods.ron"]],
      [/dimension|realm|portal|orbit/, ["crates/worldgen/src/nether.rs", "assets/star_systems.ron"]],
      [/villager|civilization|settlement|colony/, ["crates/app/src/world/travel/civilization.rs", "crates/worldgen/src/civilization.rs"]],
    ];
    const indexed = new Set(ctx.paths());
    const stopWords = new Set(["with", "that", "from", "this", "into", "feature", "native", "game", "look"]);
    const words = [...new Set(task.toLowerCase().match(/[a-z0-9_]{4,}/g) ?? [])].filter(word => !stopWords.has(word));
    const ranked = routes.map(([pattern, paths]) => ({
      paths,
      score: (pattern.test(task.toLowerCase()) ? 100 : 0)
        + Math.min(16, reference.toLowerCase().split("\n").filter(line => pattern.test(line)).length),
    })).filter(route => route.score > 0).sort((a, b) => b.score - a.score);
    const choices = [...new Set(ranked.flatMap(route => route.paths))].filter(path => indexed.has(path)).slice(0, 4);
    const owners = await Promise.all(choices.map(async path => {
      try {
        const doc = await ctx.load(path);
        let best = 0, line;
        doc.text.split("\n").forEach((text, index) => {
          const score = words.filter(word => text.toLowerCase().includes(word)).length;
          if (score > best) { best = score; line = index + 1; }
        });
        return { path, line: doc.text.length > maxExcerpt ? line : undefined };
      } catch { return null; }
    }));
    owners.filter(Boolean).forEach(attach);
  }

  function renderSources() {
    repeat.hidden = mode.value !== "repair";
    if (repeat.hidden && passes.value === "4") passes.value = "3";
    campaign.disabled = busy;
    gather.disabled = busy;
    add.disabled = busy || attached.length >= maxSources;
    importMod.disabled = pack.disabled = textures.disabled = improve.disabled = busy;
    stage.disabled = busy || ["toolkit", "audit"].includes(mode.value);
    evidenceOnRun.disabled = busy;
    copy.disabled = busy || (!attached.length && !["port", "pack", "audit", "repair"].includes(mode.value));
    run.disabled = copy.disabled || queueRunning;
    queueStart.disabled = busy || queueRunning;
    request.disabled = mode.disabled = passes.disabled = busy;
    importButton.disabled = busy || !attached.length || mode.value === "audit";
    referencePreview.hidden = !["port", "pack"].includes(mode.value);
    artPreview.hidden = mode.value !== "port";
    renderContext();
    sourceList.replaceChildren(...(attached.length ? attached.map((source, index) => {
      const remove = button("×", () => {
        attached = attached.filter(item => item !== source);
        renderSources();
      }, "Remove " + source.label);
      remove.disabled = busy;
      remove.setAttribute("aria-label", remove.title);
      const preview = el("details", { className: "agent-excerpt" },
        el("summary", {}, el("kbd", {}, "S" + (index + 1)), source.label));
      preview.ontoggle = async () => {
        if (!preview.open) return;
        try { preview.querySelector("pre")?.remove(); preview.append(el("pre", {}, (await excerpt(source)).text)); }
        catch (error) { ctx.note(error.message, true); }
      };
      return el("div", { className: "agent-source" },
        el("div", {}, preview, el("small", {}, source.path)), remove);
    }) : [muted("No Attached Sources")]));
  }
  function attach(spec) {
    if (busy || !canAssist(spec.path)) return;
    const key = JSON.stringify([spec.path, spec.parts, spec.name, spec.line, spec.spriteId, spec.valueOnly]);
    if (attached.some(source => source.key === key)) return;
    if (attached.length >= maxSources) return ctx.note("Scope limit · 5 excerpts", true);
    attached.push({ ...spec, key, label: String(spec.label ?? name(spec.path)).slice(0, 120) });
    renderSources();
  }

  function prepareFollowUp(sources, previousOutcome, preferredId, originalTask = "") {
    if (busy || queueRunning) return ctx.note("Finish or stop the active AI run first.", true);
    const choices = sources.filter(source => canAssist(source.path));
    const pick = el("select", {}, [option("", "Choose a source after preparing"),
      ...choices.map((source, index) => option(String(index), source.label ? source.label + " · " + source.path : source.path))]);
    const preferred = choices.findIndex(source => source.id === preferredId);
    if (choices.length) pick.value = String(Math.max(0, preferred));
    const feedback = el("textarea", { rows: 4, maxLength: 1000, placeholder: "What should change, or what did the last run miss?" });
    ctx.dialog("Continue with feedback", [labeled("Source for this pass", pick), labeled("Your feedback", feedback),
      muted("Prepares one pass using the selected source. You can review the brief and add context before running.")],
    "Prepare follow-up", () => {
      const detail = feedback.value.trim();
      if (!detail) { feedback.focus(); throw new Error("Describe what should change in this pass."); }
      const source = pick.value === "" ? undefined : choices[Number(pick.value)];
      attached = [];
      knowledge = contextQuery = undefined;
      evidenceOnRun.checked = false;
      mode.value = "refine";
      passes.value = "1";
      request.value = [
        "Follow-up instruction: " + detail,
        originalTask && "Previous brief: " + originalTask.slice(0, 800),
        previousOutcome && "Previous outcome (reference data): " + previousOutcome.slice(0, 360),
      ].filter(Boolean).join("\n\n");
      if (source) {
        const { path, label, parts, name, line, spriteId, valueOnly } = source;
        const savedLine = typeof source.original === "string" && Number.isInteger(source.start)
          ? source.original.slice(0, source.start).split("\n").length : undefined;
        attach({ path, label, parts, name, line: line ?? savedLine, spriteId, valueOnly });
      }
      renderSources();
      return true;
    });
  }
  async function excerpt(spec) {
    const doc = await ctx.load(spec.path);
    if (!doc.text) throw new Error("No editable text · " + spec.path);
    let start = 0, end = doc.text.length;
    if (spec.spriteId) {
      const entry = readSheet(doc.text, doc.path).entries.find(row => row.id === spec.spriteId);
      if (!entry) throw new Error("Sprite moved · " + spec.label);
      start = entry.palette[1]?.line ?? entry.rows[0].start;
      end = entry.end;
    } else if (["ron", "json"].includes(doc.kind) && spec.parts?.length) {
      const found = resolve(ctx.tree(doc), spec.parts);
      if (!found) throw new Error("Record moved · " + spec.label);
      start = spec.valueOnly ? found.node.start : found.node.entry ?? found.node.start;
      end = found.node.end;
    } else if (spec.name && spec.path.endsWith(".rs")) {
      const token = rustTunables(doc.text).find(field => field.name === spec.name);
      if (token) {
        start = doc.text.lastIndexOf("\n", token.start - 1) + 1;
        end = doc.text.indexOf("\n", token.start);
        if (end < 0) end = doc.text.length;
      }
    } else if (spec.line) {
      const rows = doc.text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
      const first = Math.max(0, spec.line - 16);
      start = rows.slice(0, first).reduce((size, row) => size + row.length, 0);
      end = start + rows.slice(first, first + 65).join("").length;
    }
    if (spec.valueOnly && end - start > maxExcerpt) throw new Error("Choose a smaller field for AI assistance.");
    end = Math.min(end, start + maxExcerpt);
    return { ...spec, doc, original: doc.text, start, text: doc.text.slice(start, end) };
  }
  const snapshot = async () => (await Promise.all(attached.map(excerpt)))
    .map((source, i) => ({ ...source, id: "S" + (i + 1) }));
  const publicSources = (sources, saved = false) => sources.map(({ id, label, path, text, doc, start }) =>
    ({ id, label, path, text, anchor: saved && !ctx.dirty(doc) ? { start, version: doc.version } : undefined }));

  async function runProposal() {
    if (busy) return;
    if (!request.value.trim()) { request.focus(); return ctx.note("Enter a task", true); }
    const task = request.value.trim(), selectedMode = mode.value;
    busy = true;
    const previousStatus = status.textContent;
    status.textContent = "Preparing run…";
    renderSources();
    try {
      assertTask(task);
      if (["audit", "repair"].includes(selectedMode) || (evidenceOnRun.checked && !contextIsCurrent())
          || (!contextIsCurrent() && attached.some(source => source.path.startsWith("docs/axiom-notes/")))) {
        await collect(task, contextQuery?.task === task ? contextQuery.symbols : []);
      }
      capture = await snapshot();
      stageOnComplete = stage.checked && !["toolkit", "audit"].includes(selectedMode);
      const state = await ctx.api("/api/agent", undefined, { task, provider: provider.value, model: model.value.trim(),
        passes: selectedMode === "repair" ? Math.max(2, Number(passes.value)) : Number(passes.value),
        mode: selectedMode, sources: publicSources(capture, true),
        reference: reference(), contextId: contextIsCurrent() ? knowledge.id : undefined });
      submittedTask = task;
      try { sessionStorage.setItem(submittedTaskKey, task); } catch { /* Private window. */ }
      proposal = undefined;
      resultBody.replaceChildren(muted("Waiting for Proposal"));
      active = state.id;
      stop.disabled = false;
      poll();
    } catch (error) {
      status.textContent = previousStatus;
      ctx.note(error.message, true);
      busy = false;
      renderSources();
    }
  }
  async function poll() {
    clearTimeout(polling);
    try {
      const state = await ctx.api("/api/agent?run=" + encodeURIComponent(active));
      if (state.id !== active) throw new Error("AI run changed");
      status.textContent = state.phase === "Running" ? (state.step ? state.step + " · " : "") + "Pass " + state.pass + " / " + state.passes
        : [state.phase, state.usage?.input ? state.usage.input + " input tokens / " + state.usage.output + " output tokens"
          : promptSize(state.promptBytes)].filter(Boolean).join(" · ");
      if (state.phase === "Running") { polling = setTimeout(poll, 750); return; }
      if (state.autoSources) capture = await loadRunSources(state.autoSources);
      if (state.proposal || state.finding?.path) {
        show(state.proposal ?? { summary: state.error ?? state.phase, checks: [], edits: [] },
          stageOnComplete, state.proposal ? state.id : undefined, state.evidence, state.attempts, state.finding, state.checkId, state.regression);
      }
      else resultBody.replaceChildren(evidenceView(state.evidence, state.attempts) ?? muted(state.phase));
      if (state.error) ctx.note(state.error, true);
    } catch (error) { ctx.note(error.message, true); }
    active = undefined;
    busy = false;
    stop.disabled = true;
    renderSources();
  }
  function show(value, autoStage = false, runId, evidence, attempts, finding, checkId, regression) {
    proposal = value;
    const snapshot = capture;
    let previews = [];
    if (value.edits.some(edit => isSheet(snapshot.find(source => source.id === edit.source)?.path ?? ""))) {
      try { previews = spriteComparisons(value, snapshot); }
      catch (error) { previews = [muted("Sprite preview unavailable · " + error.message)]; }
    }
    const canAutoStage = receipt => receipt?.status === "Syntax checked";
    const review = button("Stage Drafts", () => stageDrafts(false), "", "primary");
    function candidateControls(changes) {
      const revisions = new Map([["AI candidate", changes.map(change => change.text)]]);
      const versions = changes.map(({ doc }) => doc.version);
      const pick = el("select", {}, option("AI candidate", "AI candidate"));
      const current = () => changes.map(({ doc }) => doc.text);
      const ensureCurrent = () => {
        if (changes.some(({ doc }, i) => doc.version !== versions[i])) throw new Error("A source was saved or reloaded. Start a new candidate to make more variations.");
      };
      function remember(name) {
        if (revisions.size >= 8) throw new Error("This candidate already holds eight variations. Review the draft you want to keep.");
        revisions.set(name, current());
        pick.append(option(name, name));
      }
      function preserveWorkingDraft() {
        const texts = current();
        if (![...revisions.values()].some(saved => saved.every((text, i) => text === texts[i]))) {
          let index = revisions.size;
          while (revisions.has("Variation " + index)) index++;
          remember("Variation " + index);
        }
      }
      pick.onchange = () => {
        try {
          ensureCurrent();
          preserveWorkingDraft();
          const texts = revisions.get(pick.value);
          changes.forEach(({ doc }, i) => ctx.edit(doc, texts[i], true));
          ctx.note("Variation loaded into the visual editors");
        } catch (error) { ctx.note(error.message, true); }
      };
      const variant = button("Keep Variation", () => {
        const title = el("input", { type: "text", maxlength: 48, placeholder: "Variant name" });
        ctx.dialog("Keep current tuning", [title], "Keep", () => {
          ensureCurrent();
          const label = title.value.trim();
          if (!label || revisions.has(label)) throw new Error("Choose a new variation name");
          remember(label);
          pick.value = label;
          return true;
        });
      });
      const refine = button("Refine Current Draft", () => {
        const next = el("textarea", { rows: 4, placeholder: "What should change in this variation?" });
        ctx.dialog("Refine current candidate", [next], "Prepare AI scope", () => {
          ensureCurrent();
          if (!next.value.trim()) throw new Error("Describe the change you want");
          attached = [];
          for (const { doc, locations } of changes) {
            const line = doc.text.slice(0, locations[0]).split("\n").length;
            attach({ path: doc.path, line, label: name(doc.path) + " · current draft" });
          }
          request.value = next.value.trim();
          stage.checked = false;
          renderSources();
          request.focus();
          return true;
        });
      });
      return el("section", { className: "candidate-tuning" },
        el("h3", {}, "Tune Candidate"),
        el("div", { className: "agent-actions" }, pick, variant, refine,
          button("Review Current Bundle", () => {
            const edits = changes.filter(({ doc }) => ctx.dirty(doc)).map(({ doc }) => ({ doc, text: doc.text }));
            if (!edits.length) return ctx.note("No unsaved candidate drafts");
            const original = edits.length === changes.length && changes.every(({ doc, text }) => doc.text === text);
            ctx.reviewAgentBundle(edits, original ? runId : undefined);
          })),
        ...changes.map(({ doc, locations }) => el("div", { className: "draft-row" },
          el("strong", {}, name(doc.path)),
          ...[...new Set(locations)].map((offset, i) => button(locations.length > 1 ? "Tune Change " + (i + 1) : "Open Editor",
            () => ctx.openCandidateTarget(doc, offset).catch(error => ctx.note(error.message, true)))),
          button("Review File", () => ctx.review(doc)))));
    }
    async function stageDrafts(automatic) {
      review.disabled = true;
      try {
        if (runId) {
          const checked = await ctx.api("/api/agent/verify", undefined, { id: runId });
          if (["Stale", "Failed"].includes(checked.status)) throw new Error(checked.files[0]?.detail ?? checked.status);
          if (automatic && !canAutoStage(checked)) {
            if (!checkId) return ctx.note("Candidate needs verification before automatic staging. Review the proposal to stage it manually.");
            const receipt = await ctx.api("/api/improvement/check", undefined, { id: runId, checkId });
            if (!["Regression checked", "Native checks passed"].includes(receipt.status)) {
              return ctx.note("Registered verification did not pass. Review the candidate before staging.", true);
            }
          }
        }
        if (proposal !== value) return;
        const changes = prepareProposal(value, snapshot);
        for (const { doc, text } of changes) {
          if (["ron", "json"].includes(doc.kind)) {
            const parsed = ctx.parse(text, doc.kind);
            const field = snapshot.find(source => source.doc === doc && source.valueOnly);
            if (field) {
              const node = resolve(parsed, field.parts)?.node;
              const end = field.start + field.text.length + text.length - field.original.length;
              if (node?.start !== field.start || node.end !== end) {
                throw new Error("The proposed edit must stay inside the selected value.");
              }
            }
          }
          if (isSheet(doc.path)) readSheet(text, doc.path);
          if (doc.path === fabricationPath) fabricationRows(text);
        }
        for (const { doc, text } of changes) ctx.edit(doc, text);
        review.dataset.staged = "true";
        ctx.note(changes.length + " Drafts Staged");
        resultBody.append(candidateControls(changes));
      } catch (error) { ctx.note(error.message, true); }
      finally { if (review.isConnected && !review.dataset.staged) review.disabled = false; }
    }
    const sourceGap = !value.edits.length && value.checks.find(check =>
      /^(?:Missing source context|Owner gap)\s*:/i.test(check));
    const proposedChecks = value.checks.filter(check => check !== sourceGap);
    const checkStatus = el("span", { className: "muted", role: "status" }, regression?.status);
    const runCheck = checkId ? button("Run baseline and candidate check", async () => {
      runCheck.disabled = true;
      checkStatus.textContent = "Checking baseline and candidate…";
      try {
        const receipt = await ctx.api("/api/improvement/check", undefined, { id: runId, checkId });
        checkStatus.textContent = receipt.status + " · Baseline failures " + (receipt.baseline.fail ?? "unknown")
          + " · Candidate passes " + (receipt.candidate.pass ?? "unknown");
      } catch (error) { checkStatus.textContent = error.message; }
      finally { runCheck.disabled = false; }
    }) : null;
    resultBody.replaceChildren(...(finding?.path ? [el("details", { open: true, className: "agent-edit" },
      el("summary", {}, "Selected defect · " + finding.path), el("p", {}, finding.summary),
      el("p", {}, "Trigger · " + finding.trigger), el("p", {}, "Impact · " + finding.impact),
      el("p", {}, "Verification · " + finding.check), button("Open Defect", async () => {
        try {
          const source = snapshot.find(row => row.id === finding.source);
          if (!source || source.doc.text !== source.original ||
              (source.anchor && source.doc.version !== source.anchor.version)) {
            throw new Error("The defect location has changed. Run the audit again on the current source.");
          }
          await ctx.openCandidateTarget(source.doc, source.start + finding.offset);
        } catch (error) { ctx.note(error.message, true); }
      }))] : []),
      ...(sourceGap ? [
        el("h3", {}, "Needs source context"),
        el("p", {}, sourceGap.replace(/^[^:]+:\s*/, "")),
        button("Gather Context", gatherContext, "Find the missing source and attach it before running again", "primary"),
        el("details", { className: "agent-edit" }, el("summary", {}, "Proposal summary"), el("p", {}, value.summary)),
      ] : [el("p", {}, value.summary)]),
      ...previews, evidenceView(evidence, attempts) ?? muted("Imported proposal · native verification pending"),
      ...(runId ? [button("Continue with feedback", () =>
        prepareFollowUp(snapshot, value.summary, value.edits[0]?.source, request.value.trim()))] : []),
      ...(runCheck ? [el("div", { className: "agent-actions" }, runCheck, checkStatus)] : []),
      ...(proposedChecks.length ? [el("details", { className: "agent-edit" },
        el("summary", {}, "Proposed checks (" + proposedChecks.length + ")"),
        el("ul", { className: "agent-checks" }, proposedChecks.map(check => el("li", {}, check))))] : []),
      ...value.edits.map(edit => {
        const file = capture.find(row => row.id === edit.source);
        return el("details", { className: "agent-edit" }, el("summary", {}, file.label + " · " + edit.reason),
          el("div", { className: "agent-diff" }, el("pre", { className: "removed" }, edit.before),
            el("pre", { className: "added" }, edit.after)));
      }), ...(value.edits.length ? [review] : []));
    if (autoStage && value.edits.length) stageDrafts(true);
    if (runId) {
      const actions = el("div", { className: "agent-actions" });
      const feedback = outcome => {
        const detail = el("textarea", { rows: 3, maxLength: 350, placeholder: "What worked or what needs repair?" });
        ctx.dialog("Record outcome", [detail], "Record", async () => {
          await ctx.api("/api/agent/review", undefined, { id: runId, outcome, note: detail.value });
          actions.replaceChildren(muted("Outcome recorded"));
          return true;
        });
      };
      actions.append(button("Useful", () => feedback("useful")), button("Needs Repair", () => feedback("needs-repair")));
      resultBody.append(actions);
    }
  }

  renderSources();
  void refreshQueue();
  return {
    createMission({ task, paths = [], parityId }) {
      openMissionComposer({ task, paths: paths.filter(canAssist), parityId, provider: "auto" });
    },
    openMission(id) { showView(true); missionPanel.open(id); },
    attach,
    /** Contextual handoffs use a fresh scope, with a deliberate draft replacement. */
    handoff({ task, sources = [], paths = [], mode: nextMode, origin, parityId }, ready) {
      if (busy) return ctx.note("Finish or stop the active AI run before opening a new task.", true);
      if (typeof task !== "string" || !task.trim()) return;
      const load = () => {
        if (busy) return false;
        showView(false);
        parityTaskLink.hidden = typeof parityId !== "string" || !/^[A-Z][A-Z0-9-]{0,47}$/.test(parityId);
        currentParityId = parityTaskLink.hidden ? undefined : parityId;
        if (!parityTaskLink.hidden) {
          parityTaskLink.href = "/parity?id=" + encodeURIComponent(parityId) + location.hash;
          parityTaskLink.textContent = "Parity task " + parityId;
        } else parityTaskLink.removeAttribute("href");
        attached = [];
        knowledge = undefined;
        proposal = undefined;
        capture = [];
        resultBody.replaceChildren(muted("Ready to run"));
        status.textContent = "Ready";
        modReference = toolkitReference = textureReference = modelReference = "";
        modTexture = undefined;
        referencePreview.replaceChildren();
        artPreview.textContent = "";
        mode.value = ["refine", "audit", "systems"].includes(nextMode) ? nextMode : "refine";
        passes.value = "1";
        stage.checked = false;
        evidenceOnRun.checked = true;
        request.value = task;
        for (const source of [...sources, ...paths.map(path => ({ path }))].slice(0, maxSources)) {
          if (canAssist(source?.path)) attach(source);
        }
        renderSources();
        ready?.(origin);
        request.focus();
        return true;
      };
      if (request.value.trim() && request.value.trim() !== submittedTask && request.value.trim() !== task.trim()) {
        ctx.dialog("Replace the current AI task?", [
          muted("This loads the new task and its selected sources. Saved run history is available in the workbench."),
        ], "Replace task", load);
      } else load();
    },
    focusField(spec, task) {
      if (busy) return ctx.note("An AI run is already active.", true);
      showView(false);
      attached = [];
      mode.value = "refine";
      stage.checked = false;
      request.value = task;
      attach(spec);
      return true;
    },
    focusTask() {
      showView(false);
      request.focus();
      request.setSelectionRange(request.value.length, request.value.length);
    },
  };
}

export function sourceFromDocument(doc, line) {
  return { path: doc.path, label: name(doc.path), line,
    parts: !line && doc.trail?.length && doc.parsed?.children ? address(doc.parsed, doc.trail) : undefined };
}
