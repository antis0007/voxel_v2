import { el, button } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";
import { maxTaskLength } from "./agent-contract.mjs";

const muted = text => el("p", { className: "muted" }, text);
const field = (label, control) => el("label", { className: "stack" }, label, control);
const option = (value, label = value) => el("option", { value }, label);
const lines = value => value.split(/\r?\n/).map(row => row.trim()).filter(Boolean);
const input = (value, attrs = {}) => el("input", { type: "text", value, ...attrs });
const missionTitle = objective => {
  const line = objective.trim().split(/\n|(?<=[.!?])\s/)[0];
  return line.length > 130 ? line.slice(0, 127).replace(/\s+\S*$/, "") + "…" : line;
};
const metric = (label, value) => el("div", {}, el("dt", {}, label), el("dd", {}, value));
const finished = new Set(["Saved", "Accepted", "Reported"]);

const outcomeLabel = mission => mission.phase !== "Complete" ? mission.phase
  : mission.outcome ?? (!Array.isArray(mission.steps) ? "Finished"
  : mission.steps.some(step => step.status === "Saved") ? "Slices saved"
  : mission.steps.every(step => step.status === "Reported") && mission.steps.length ? "Report finished" : "Plan finished");
function nextAction(mission) {
  if (!["Blocked", "Deferred", "Interrupted"].includes(mission.phase)) return;
  const reason = mission.error ?? "";
  if (/source (target )?changed|shared input changed|re-pin/i.test(reason)) return "Review the changed files, then re-pin the plan or create a scoped follow-up. Retrying unchanged evidence will not resolve this.";
  if (/verification code|existing tests|imports, attributes|indexed implementation files/i.test(reason)) return "Correct the source selection or proposal while preserving verification code. Keep the test protections; do not simply repeat the rejected edit.";
  if (/missing source|source context|owning specification/i.test(reason)) return "Resolve the missing production owner or specification named above, then continue with that evidence.";
  if (/budget|attempt.*exhaust|time.*limit/i.test(reason) || mission.attempts >= mission.settings.maxAttempts) return "Review what the attempts established. Narrow the unfinished slice before increasing its budget; saved work is retained.";
  if (mission.phase === "Interrupted") return "Check whether the owning worker is still active before recovering its lane. Do not restart an active job.";
  return "Read the last failure and candidate evidence before continuing. Resolve that condition first; continuing is not a guarantee of recovery.";
}

export function mountWorkflow(host, ctx, current) {
  let selected, timer, pending = false, toolingOwners = [], refreshVersion = 0;
  let workflowStorageVersion = 0, visibleMissions = [], pickerKey, detailKey, validationKey, overviewKey;
  let selectionKey, detailLoadedAt = 0, validationLoadedAt = 0, validationPending = false;
  const writable = mission => workflowStorageVersion === 2 && !mission.legacy && !mission.readOnly;
  async function mutate(input) {
    if (workflowStorageVersion !== 2) throw new Error("This panel is served by an older workflow worker. Use an updated panel before changing missions; existing work is preserved.");
    if (input.id && !visibleMissions.some(mission => mission.id === input.id && writable(mission))) {
      throw new Error("This mission is preserved history, not an executable queue entry. Create a fresh, scoped follow-up using its evidence.");
    }
    return ctx.api("/api/workflows", undefined, { ...input, workflowStorageVersion: 2 });
  }
  const overview = el("div", { className: "mission-queue-status", "aria-label": "Queue status", "aria-live": "polite" }, muted("Connecting to the worker…"));
  const validation = el("details", { className: "mission-disclosure mission-validation" },
    el("summary", {}, "Published change validation"), muted("Loading verification…"));
  validation.addEventListener("toggle", () => { if (validation.open) void refreshValidation(); });
  const body = el("div", { className: "mission-body" });
  const picker = el("select", { "aria-label": "Mission" });
  const filter = el("select", { "aria-label": "Filter missions" },
    option("all", "All missions"), option("running", "Active"), option("queued", "Queued"),
    option("attention", "Needs attention"), option("finished", "Finished and history"));
  filter.onchange = () => { selected = undefined; detailKey = undefined; void refresh(); };
  const matchesFilter = mission => filter.value === "all"
    || filter.value === "running" && ["Running", "Planning"].includes(mission.phase)
    || filter.value === "queued" && mission.phase === "Queued"
    || filter.value === "attention" && ["Blocked", "Deferred", "Interrupted", "Paused"].includes(mission.phase)
    || filter.value === "finished" && (mission.archived || mission.legacy || mission.readOnly || ["Complete", "Cancelled"].includes(mission.phase));

  const recover = button("Recover abandoned AI lane", async () => {
    try { await mutate({ action: "recover-lane" }); await refresh(); }
    catch (error) { ctx.note(error.message, true); }
  });
  recover.hidden = true;
  picker.onchange = () => { selected = picker.value; void refresh(); };
  function openCreate(source = current()) {
    if (create.disabled) return ctx.note("Mission setup is unavailable. Wait for the queue to reconnect.", true);
    const objective = el("textarea", { rows: 4, value: source.task, required: true,
      "aria-label": "Mission outcome", placeholder: "Describe the result this mission should deliver." });
    const count = el("span", { className: "muted", "aria-live": "polite" });
    const validateObjective = () => {
      const length = objective.value.length;
      count.textContent = length + " / " + maxTaskLength + " characters";
      objective.setCustomValidity(!objective.value.trim() ? "Enter the mission outcome." : length > maxTaskLength
        ? "The mission outcome has " + length + " characters; shorten it to " + maxTaskLength + " or fewer. Your text has been kept." : "");
    };
    objective.addEventListener("input", validateObjective);
    validateObjective();
    const paths = el("textarea", { rows: 4, value: source.paths.join("\n") });
    const controls = settings({ provider: source.provider, model: source.model, publication: "automatic",
      checks: source.parityId ? "build" : "compile" });
    ctx.dialog(source.parityId ? "Create mission for " + source.parityId : "Create an automatic mission", [field("Outcome", objective), count, field("Source hints · optional, one per line, up to five", paths),
      el("details", { className: "mission-disclosure" }, el("summary", {}, "Provider, verification and budget"),
        el("div", { className: "mission-detail" }, controls.elements))],
    "Create mission", async () => {
      validateObjective();
      if (!objective.reportValidity()) return false;
      const mission = await mutate({ action: "create", objective: objective.value.trim(),
        paths: lines(paths.value), settings: controls.value(), parityId: source.parityId });
      selected = mission.id;
      filter.value = "all";
      heading.closest(".mission-panel").scrollIntoView({ block: "start" });
      void refresh();
      return true;
    });
  }
  const create = button("New mission", () => openCreate());
  create.classList.add("primary");
  create.disabled = true;
  const improve = button("Improve dev tools", () => {
    const source = current();
    const problem = el("textarea", { rows: 4, required: true, maxlength: 1350,
      value: source.task.length <= 1350 ? source.task : "",
      placeholder: "Describe an observed problem and how to reproduce it." });
    const owner = el("select", {}, ...toolingOwners.map(path => option(path, path.split("/").at(-1))));
    ctx.dialog("Queue a verified tool improvement", [field("Observed problem", problem), field("Tool", owner),
      muted("Uses your current AI provider and up to two focused attempts. Runs the registered check before and after the edit and saves a rollback bundle if it reproduces and fixes the failure.")],
    "Run improvement and checks", async () => {
      if (!problem.reportValidity()) return false;
      const mission = await mutate({ action: "create-improvement", path: owner.value,
        objective: problem.value, settings: { provider: source.provider, model: source.model } });
      selected = mission.id;
      void refresh();
      return true;
    });
  });
  improve.hidden = true;
  const heading = el("h2", { tabIndex: -1 }, "Automatic missions");
  const section = el("section", { id: "agent-missions", className: "craft-window mission-panel", hidden: true, "aria-label": "Automatic missions" },
    el("header", { className: "mission-toolbar" }, heading,
      el("div", { className: "agent-actions" }, create, improve, button("Refresh", () => refresh(true)), recover)), overview,
    el("div", { className: "mission-picker" }, field("Show", filter), field("Mission", picker)), body, validation);
  host.append(section);

  function settings(value = {}) {
    const provider = el("select", {}, option("auto", "Claude Code + Codex"), option("claude", "Claude Code"),
      option("codex", "Codex"), option("ollama", "Ollama"), option("lmstudio", "LM Studio"));
    provider.value = value.provider ?? "claude";
    const model = input(value.model ?? "", { maxLength: 100, placeholder: "Provider default" });
    model.disabled = provider.value === "auto";
    provider.addEventListener("change", () => { model.disabled = provider.value === "auto"; });
    const publication = el("select", {}, option("review", "Review each candidate"), option("automatic", "Save eligible candidates and continue"));
    publication.value = value.publication ?? "automatic";
    const checks = el("select", {}, option("build", "Build Axiom · isolated release-fast"),
      option("compile", "Structural and Rust compile checks · no tests"),
      option("syntax", "Structural checks only · no tests"), option("registered", "Run registered behavioral checks"));
    checks.value = value.checks ?? (publication.value === "automatic" ? "compile" : "syntax");
    const attempts = input(value.maxAttempts ?? 12, { type: "number", min: 1, max: 36 });
    const minutes = input(value.maxMinutes ?? 30, { type: "number", min: 1, max: 180 });
    return { elements: [el("div", { className: "agent-options" }, field("Provider", provider), field("Model", model)),
      field("Publication", publication), field("Verification", checks),
      muted("The game build compiles and links a candidate in isolation. Automatic saving creates a reversible bundle after validation."),
      el("div", { className: "agent-options" }, field("Maximum step attempts", attempts), field("Total active minutes", minutes))],
    value: () => ({ provider: provider.value, model: model.disabled ? "" : model.value.trim(), publication: publication.value, checks: checks.value,
      maxAttempts: Number(attempts.value), maxMinutes: Number(minutes.value) }) };
  }

  async function command(mission, action, extra = {}) {
    if (pending) return;
    pending = true;
    try {
      const result = await mutate({ action, id: mission.id, revision: mission.revision, ...extra });
      selected = result.id;
    } catch (error) { ctx.note(error.message, true); }
    finally { pending = false; await refresh(); }
  }

  function editPlan(mission) {
    const list = el("div", { className: "stack" });
    const rows = [];
    function add(step = {}) {
      const title = input(step.title ?? "", { maxLength: 120 });
      const task = el("textarea", { rows: 3, value: step.task ?? "" });
      const acceptance = el("textarea", { rows: 2, value: step.acceptance ?? "", maxLength: 700 });
      const paths = el("textarea", { rows: 3, value: (step.paths ?? mission.paths).join("\n") });
      const mode = el("select", {}, option("refine", "Implement"), option("repair", "Audit and repair"), option("audit", "Read-only report"));
      mode.value = step.mode ?? "refine";
      const query = el("textarea", { rows: 2, maxLength: 600, value: step.query ?? "" });
      const targets = el("textarea", { rows: 2, value: (step.targets ?? []).map(target => target.path + ":" + target.line).join("\n"),
        placeholder: "tools/axiom-dev/game.mjs:246" });
      const repin = el("input", { type: "checkbox" });
      const row = { title, task, acceptance, paths, mode, query, targets, repin, targetPins: step.targets ?? [] };
      row.node = el("details", { className: "agent-edit", open: true }, el("summary", {}, step.title ?? "New step"),
        field("Title", title), field("Task (up to 1600 characters; split large outcomes into steps)", task), field("Acceptance criteria", acceptance), field("Source files", paths), field("Mode", mode),
        el("details", {}, el("summary", {}, "Focused source context (optional)"),
          field("Retrieval query (does not replace task or acceptance)", query),
          field("Source targets: project-relative path:line, one per source file", targets),
          field("Re-pin targets to current source revisions on save (only after rechecking source)", repin)),
        button("Move up", () => { const index = rows.indexOf(row); if (index > 0) { [rows[index - 1], rows[index]] = [rows[index], rows[index - 1]]; list.replaceChildren(...rows.map(item => item.node)); } }),
        button("Remove step", () => { rows.splice(rows.indexOf(row), 1); row.node.remove(); }));
      rows.push(row); list.append(row.node);
    }
    (mission.steps.length ? mission.steps : [{ title: "Implement the outcome", task: mission.objective }]).forEach(add);
    ctx.dialog("Review ordered mission plan", [muted("Each step waits for the previous step to be saved or accepted. The plan freezes when execution begins."), list,
      button("Add step", () => { if (rows.length < 12) add(); })], "Save plan", async () => {
      if (mission.sourceContextVersion !== 1 && rows.some(row => row.query.value.trim() || row.targets.value.trim())) {
        throw new Error("The serving workflow worker does not support focused source context. Restart the panel after active work finishes, then refresh.");
      }
      const steps = rows.map(row => {
        const targets = lines(row.targets.value).map(value => {
          const match = /^(.*):([1-9]\d*)$/.exec(value.replaceAll("\\", "/"));
          if (!match) throw new Error("Source targets need project-relative path:line.");
          const target = { path: match[1], line: Number(match[2]) };
          const previous = row.targetPins.find(pin => pin.path === target.path && pin.line === target.line);
          return previous && !row.repin.checked ? { ...target, version: previous.version } : target;
        });
        return { title: row.title.value, task: row.task.value, acceptance: row.acceptance.value,
          paths: lines(row.paths.value), mode: row.mode.value,
          ...(row.query.value.trim() ? { query: row.query.value } : {}), ...(targets.length ? { targets } : {}) };
      });
      const result = await mutate({ action: "update", id: mission.id, revision: mission.revision, sourceContextVersion: 1, steps });
      selected = result.id; void refresh(); return true;
    });
  }

  function review(mission, step) {
    const candidate = step.candidate;
    const note = el("textarea", { rows: 3, maxLength: 1000, placeholder: "Review evidence and remaining limitations" });
    const hasEdits = Boolean(candidate.proposal?.edits.length);
    ctx.dialog("Review · " + step.title, [muted(step.acceptance),
      el("pre", { className: "agent-excerpt" }, candidate.proposal?.summary ?? candidate.error ?? "No proposal"),
      candidate.error ? muted(candidate.error) : null,
      muted("Recorded validation: " + (candidate.evidence?.status ?? "None") + ". Fresh source revisions are checked again before saving."),
      candidate.compile ? muted(candidate.compile.message) : null,
      ...(candidate.evidence?.files ?? []).map(file => muted(file.path + " · " + file.status + (file.detail ? " · " + file.detail : ""))),
      ...(candidate.proposal?.checks ?? []).map(check => muted("Suggested, not executed: " + check)),
      ...(candidate.proposal?.edits ?? []).map(edit => el("details", { className: "agent-edit" },
        el("summary", {}, (candidate.autoSources?.find(row => row.id === edit.source)?.path ?? edit.source) + " · " + edit.reason),
        el("div", { className: "agent-diff" }, el("pre", {}, edit.before), el("pre", {}, edit.after)))),
      field("Review record", note),
      muted(hasEdits ? "Save this candidate using revision checks and backups, then continue the mission. This records your review; it does not mark gameplay verified."
        : "Accept this step's report or no-change outcome, then continue. This does not prove the requested behavior works.")].filter(Boolean),
    hasEdits ? "Save and continue" : "Accept and continue", async () => {
      await mutate({ action: hasEdits ? "approve" : "accept", id: mission.id, revision: mission.revision, note: note.value });
      void refresh(); return true;
    });
  }

  function render(mission) {
    const readOnly = !writable(mission);
    const running = !readOnly && ["Running", "Planning", "Queued"].includes(mission.phase);
    const editable = !readOnly && !running && mission.phase !== "Interrupted";
    const controls = [];
    if (editable && !mission.steps.length && !["Complete", "Cancelled"].includes(mission.phase)) controls.push(button("Generate plan", () => command(mission, "plan"), "Propose ordered steps", "primary"));
    if (editable && !mission.improvementVersion && !mission.steps.some(row => row.attempts)) controls.push(button("Edit plan", () => editPlan(mission)));
    if (editable && mission.steps.length && !["Complete", "Cancelled"].includes(mission.phase)) controls.unshift(button("Continue automatically", () => command(mission, "resume"), "Run the existing plan", "primary"));
    if (running) controls.push(button("Pause", () => command(mission, "pause")));
    if (!readOnly && mission.phase === "Interrupted") controls.push(button("Recover interrupted run", () => command(mission, "recover")));
    if (!readOnly && !["Complete", "Cancelled"].includes(mission.phase)) controls.push(button("Cancel mission", () => command(mission, "cancel")));
    if (editable) controls.push(button("Settings and budgets", () => {
      const fields = settings(mission.settings);
      ctx.dialog("Mission settings", fields.elements, "Save settings", async () => {
        await mutate({ action: "update", id: mission.id, revision: mission.revision, settings: fields.value() });
        void refresh(); return true;
      });
    }));
    if (!readOnly && ["Complete", "Cancelled"].includes(mission.phase)) controls.push(button("Archive mission", () => command(mission, "archive")));
    const next = mission.steps.find(row => !finished.has(row.status));
    if (editable && next?.candidate) controls.unshift(button("Review candidate", () => review(mission, next), "Open evidence and proposed changes", "primary"));
    const disclosures = new Map(Array.from(body.querySelectorAll("details[data-disclosure]"),
      details => [details.dataset.disclosure, details.open]));
    if (body.dataset.mission !== mission.id) disclosures.clear();
    body.dataset.mission = mission.id;
    const disclosure = (key, title, ...content) => el("details",
      { className: "mission-disclosure", "data-disclosure": key, open: disclosures.get(key) || false },
      typeof title === "string" ? el("summary", {}, title) : title,
      el("div", { className: "mission-detail" }, ...content));
    const { finished: done, total } = mission.progress;
    const activeLabel = mission.live
      ? (mission.live.step ?? mission.phase) + " · Pass " + mission.live.pass + " of " + mission.live.passes
      : next?.title ?? (mission.phase === "Planning" ? "Building the plan" : "");
    body.replaceChildren(el("div", { className: "mission-overview" },
      el("div", { className: "mission-identity" },
        el("div", { className: "mission-state", "data-phase": mission.phase }, outcomeLabel(mission)),
        el("h3", {}, missionTitle(mission.objective)),
        activeLabel ? el("p", { className: "mission-active" }, activeLabel) : null,
        el("div", { className: "agent-actions mission-actions" }, ...controls)),
      el("div", { className: "mission-stats" },
        el("div", { className: "mission-progress" }, el("strong", {}, total ? done + " of " + total + " steps resolved" : "Plan pending"),
          total ? el("progress", { max: total, value: done, "aria-label": "Completed mission steps" }) : null),
        el("dl", {}, metric("Attempts", mission.attempts + " / " + mission.settings.maxAttempts),
          metric("Active time", Math.ceil(mission.elapsedMs / 60000) + " / " + mission.settings.maxMinutes + " min"),
          metric("Publication", mission.settings.publication === "review" ? "Review first" : "Automatic"),
          mission.usage?.input || mission.usage?.output ? metric("Tokens", mission.usage.input.toLocaleString() + " in · " + mission.usage.output.toLocaleString() + " out")
            : mission.usage?.promptBytes ? metric("Prompt text", (mission.usage.promptBytes / 1024).toFixed(1) + " KB") : null))),
      ...(readOnly ? [muted(typeof mission.recovery === "string" ? mission.recovery
        : "Preserved mission history. This entry is not queued for this worker. Its evidence stays available; create a fresh scoped follow-up rather than repeating exhausted attempts.")] : []),
      ...(mission.error ? [el("p", { className: "mission-error", role: "alert" }, mission.error)] : []),
      muted(mission.steps.filter(step => step.status === "Saved").length + " saved slices · "
        + mission.steps.filter(step => step.status === "Reported").length + " reports · "
        + mission.steps.filter(step => step.status === "Accepted").length + " accepted outcomes. These counts do not establish integrated gameplay verification."),
      ...(nextAction(mission) ? [el("div", { className: "mission-detail" }, el("strong", {}, "Next step"), muted(nextAction(mission)))] : []),
      ...(["Blocked", "Deferred"].includes(mission.phase) && mission.waitingForSource && mission.attempts < mission.settings.maxAttempts
        && mission.elapsedMs < mission.settings.maxMinutes * 60000 ? [muted("Retries automatically when the shared source changes.")] : []),
      disclosure("brief", "Mission brief", el("div", { className: "mission-copy" }, mission.objective)),
      ...(mission.steps.length ? [el("section", { className: "mission-plan", "aria-label": "Mission plan" },
        el("h4", {}, "Plan"), ...mission.steps.map(step => disclosure("step-" + step.id,
        el("summary", { "data-current": String(step.id === next?.id) },
          el("span", { className: "mission-step-index" }, String(step.order).padStart(2, "0")),
          el("strong", {}, step.title),
          el("span", { className: "mission-step-state" }, step.status)),
        el("div", { className: "mission-copy" }, step.task),
        step.acceptance ? el("div", {}, el("strong", {}, "Acceptance"), el("p", { className: "mission-copy" }, step.acceptance)) : null,
        step.paths.length ? el("ul", { className: "mission-source-list" }, ...step.paths.map(path => el("li", {}, el("code", {}, path)))) : null,
        ...(step.candidate ? [muted(step.candidate.evidence?.status ?? "Unverified")] : []),
        ...(step.candidate?.compile ? [muted(step.candidate.compile.message)] : []),
        ...(step.candidate?.providerPasses?.length
          ? [muted("Providers used · " + [...new Set(step.candidate.providerPasses.map(pass => pass.provider))].join(", "))]
          : step.candidate?.provider ? [muted("Provider used · " + step.candidate.provider)] : []),
        ...(step.evidenceState ? [muted(step.evidenceState)] : []),
        ...(step.runtime ? [el("details", {}, el("summary", {}, "Recorded runtime observation"), el("pre", {}, JSON.stringify(step.runtime, null, 2)))] : []),
        ...(step.candidate && (readOnly || step.id !== next?.id) ? [el("details", {}, el("summary", {}, readOnly ? "Preserved candidate (read-only)" : "Saved step result"),
          el("pre", {}, step.candidate.proposal?.summary ?? step.candidate.error ?? "No result"),
          ...(readOnly ? [
            ...(step.candidate.evidence?.files ?? []).map(file => muted(file.path + " - " + file.status + (file.detail ? " - " + file.detail : ""))),
            ...(step.candidate.proposal?.checks ?? []).map(check => muted("Suggested, not executed: " + check)),
            ...(step.candidate.proposal?.edits ?? []).map(edit => el("details", { className: "agent-edit" },
              el("summary", {}, (step.candidate.autoSources?.find(row => row.id === edit.source)?.path ?? edit.source) + " - " + edit.reason),
              el("div", { className: "agent-diff" }, el("pre", {}, edit.before), el("pre", {}, edit.after))))
          ] : []))] : []),
        ...(step.commit ? [el("div", {}, el("strong", {}, "Commit "), el("code", {}, step.commit))] : []),
        ...(step.bundleId ? [muted("Saved bundle " + step.bundleId)] : []),
        ...(step.commitNote ? [muted(step.commitNote)] : []),
        ...(step.review ? [muted("Review: " + step.review.note)] : []),
        ...(editable && step.id === next?.id && step.attempts ? [button("Retry with feedback", () => {
          const reason = el("textarea", { rows: 2, maxLength: 500 });
          const task = el("textarea", { rows: 4, value: step.task });
          ctx.dialog("Retry this step", [field("Reason / evidence", reason), field("Refined task", task),
            muted("The old result stays in history. The next attempt gathers fresh context and uses the remaining mission budget.")], "Prepare retry", async () => {
            await mutate({ action: "retry", id: mission.id, revision: mission.revision, note: reason.value, task: task.value });
            void refresh(); return true;
          });
        })] : []))))] : []),
      disclosure("history", "Evidence and step history",
        ...mission.events.slice(-50).map(event => muted(event.at + " · " + event.kind + " · " + event.detail)),
        button("Copy mission handoff", async () => {
          await copyText(JSON.stringify(mission, null, 2)); ctx.note("Mission contract and evidence copied");
        })));
  }

  function renderValidation(report) {
    const key = JSON.stringify(report ?? null);
    if (key === validationKey) return;
    validationKey = key;
    if (!report) {
      validation.replaceChildren(el("summary", {}, "Published change validation"), muted("No publication verification record yet."));
      return;
    }
    const rows = report.rows ?? [];
    const benchmarks = new Map((report.benchmarks?.rows ?? []).map(bench => [bench.mission, bench]));
    const table = el("table", { className: "mission-validation-table" },
      el("thead", {}, el("tr", {}, ...["Published change", "Gameplay", "Performance"].map(label => el("th", { scope: "col" }, label)))),
      el("tbody", {}, ...rows.map(row => {
        const parity = row.id === "L026" ? report.parity : undefined;
        const measured = benchmarks.get(row.mission);
        const benchmark = measured?.bundles.join() === row.bundles.join() ? measured : undefined;
        const timing = Object.entries(benchmark?.cases ?? {}).map(([name, value]) => ({
          status: name.replaceAll("_", " "),
          detail: "p50 " + (value.before.p50Ns / 1000).toFixed(1) + " → " + (value.after.p50Ns / 1000).toFixed(1) + " µs per batch"
            + (value.before.residentBytes === undefined ? "" : " · " + value.before.residentBytes + " → " + value.after.residentBytes + " resident bytes"),
        }));
        const phase = parity?.running ? parity.phase : parity?.checks?.length
          ? parity.checks.every(check => check.pass) ? "Partial" : "Failed"
          : row.gameplay.status;
        const result = (value, detail, checks = []) => el("td", {},
          el("strong", { className: "mission-validation-status", "data-status": value }, value),
          el("p", {}, detail),
          checks.length ? el("ul", {}, ...checks.map(check => el("li", {}, check.status + (check.selector ? " · " + check.selector : "")
            + (check.passed ? " · " + check.passed + " passed" : "") + (check.detail ? " · " + check.detail : "")))) : null);
        return el("tr", {},
          el("th", { scope: "row" }, button(row.title, () => { filter.value = "all"; selected = row.mission; void refresh(); }),
            row.evidenceSource && row.evidenceSource !== report.source ? muted("Earlier snapshot " + row.evidenceSource.slice(0, 12)) : null,
            row.changedSincePublication?.length ? muted("Later edits in " + row.changedSincePublication.length + " published files") : null),
          result(phase, [row.gameplay.detail, parity?.message].filter(Boolean).join(" "), row.gameplay.checks),
          result(benchmark?.status ?? row.performance.status, benchmark
            ? [benchmark.detail, benchmark.error].filter(Boolean).join(" ") : row.performance.detail,
          [...timing, ...(row.performance.diagnostic ? [row.performance.diagnostic] : [])]));
      })));
    validation.replaceChildren(
      el("summary", {}, "Published change validation"),
      el("div", { className: "mission-validation-content" },
        el("p", { className: "mission-validation-phase", role: "status" },
          report.active ? report.phase : ["Checks finished", "Build blocked", "Blocked", "Evidence unavailable"].includes(report.phase)
            ? report.phase : "Worker stopped during " + report.phase),
        report.blocker ? el("p", { className: "mission-error" }, report.blocker) : null,
        report.source ? muted("Frozen source " + report.source.slice(0, 12) + " · " + rows.length + " published changes") : null,
        rows.length ? el("div", { className: "mission-validation-scroll", tabIndex: 0 }, table) : null,
        report.runId ? button("Copy verification record", async () => {
          await copyText(JSON.stringify(report, null, 2)); ctx.note("Verification record copied");
        }) : null));
  }

  async function refreshValidation() {
    if (validationPending) return;
    validationPending = true;
    try {
      const state = await ctx.api("/api/workflows?view=validation");
      renderValidation(state.validation);
    } catch (error) {
      validationKey = undefined;
      validation.replaceChildren(el("summary", {}, "Published change validation"), muted(error.message), button("Retry", refreshValidation));
    } finally {
      validationLoadedAt = Date.now();
      validationPending = false;
    }
  }

  const visible = () => !document.hidden && section.isConnected && !section.closest("[hidden]");
  async function refresh(force = false) {
    clearTimeout(timer);
    timer = undefined;
    if (!visible()) return;
    const version = ++refreshVersion;
    try {
      const state = await ctx.api("/api/workflows?view=summary");
      if (version !== refreshVersion) return;
      workflowStorageVersion = state.workflowStorageVersion ?? 0;
      visibleMissions = state.missions;
      const live = visibleMissions.filter(writable);
      const count = phase => live.filter(mission => mission.phase === phase).length;
      const preserved = visibleMissions.length - live.length;
      create.disabled = improve.disabled = recover.disabled = workflowStorageVersion !== 2;
      const totals = [[count("Running") + count("Planning"), "Active", "running"], [count("Queued"), "Queued", "queued"],
        [count("Deferred") + count("Blocked") + count("Interrupted") + count("Paused"), "Needs attention", "attention"],
        [count("Complete") + count("Cancelled") + preserved + (state.archivedCount ?? 0), "History", "finished"]];
      const activity = live.find(row => ["Running", "Planning"].includes(row.phase));
      const worker = workflowStorageVersion !== 2 ? "Update the worker to change missions."
        : state.lane?.alive === false ? "Worker interrupted"
        : activity ? missionTitle(activity.objective)
        : state.lane?.label ?? live.find(row => row.phase === "Queued")?.error ?? "Worker idle";
      const autonomy = state.autonomy;
      const nextOverviewKey = JSON.stringify([totals, worker, autonomy]);
      if (nextOverviewKey !== overviewKey) {
        overview.replaceChildren(
          el("div", { className: "mission-totals" }, totals.map(([total, label, group]) =>
            el("button", { type: "button", onclick: () => {
              filter.value = group; filter.onchange();
            } }, el("strong", {}, String(total)), el("span", {}, label)))),
          el("div", { className: "mission-worker" }, el("span", { className: "mission-state" },
            autonomy?.enabled ? "Autonomy on" : "Manual intake"), el("span", {}, worker)),
          autonomy?.version === 2 ? el("div", { className: "mission-worker" },
            button(autonomy.enabled ? "Pause parity intake" : "Start parity intake", async () => {
              try { await mutate({ action: "autonomy", enabled: !autonomy.enabled }); await refresh(); }
              catch (error) { ctx.note(error.message, true); }
            }),
            el("span", {}, autonomy.eligible + " eligible"
              + (autonomy.next[0] ? " · Next " + autonomy.next[0].id + " " + autonomy.next[0].label : ""))) : null);
        overviewKey = nextOverviewKey;
      }
      toolingOwners = state.toolingCheckOwners ?? [];
      if (validation.open && (force || Date.now() - validationLoadedAt >= 10000)) void refreshValidation();
      improve.hidden = !toolingOwners.length;
      recover.hidden = !state.lane || state.lane.alive;
      const listed = new Set(state.missions.map(row => row.id));
      const selectable = [...state.missions, ...(state.recentArchive ?? []).filter(row => !listed.has(row.id))
        .map(row => ({ ...row, readOnly: true, archived: true }))].filter(matchesFilter);
      if (!selectable.some(row => row.id === selected)) selected = selectable.find(row => ["Running", "Planning"].includes(row.phase))?.id ?? selectable[0]?.id;
      const options = selectable.map(row => [row.id, (row.archived ? "Archive · " : row.legacy || row.readOnly ? "History · " : "") + outcomeLabel(row) + " · " + missionTitle(row.objective)]);
      const nextPickerKey = JSON.stringify(options);
      if (nextPickerKey !== pickerKey) {
        picker.replaceChildren(...options.map(([id, label]) => option(id, label)));
        pickerKey = nextPickerKey;
      }
      picker.hidden = !selectable.length;
      const row = selectable.find(row => row.id === selected);
      const nextSelectionKey = JSON.stringify([row?.id, row?.revision, row?.phase, row?.live, row?.stopping, filter.value]);
      if (row) {
        picker.value = selected;
        if (force || nextSelectionKey !== selectionKey || Date.now() - detailLoadedAt >= 15000) {
          if (body.dataset.mission !== row.id) body.replaceChildren(muted("Loading mission…"));
          const mission = await ctx.api("/api/workflows?id=" + encodeURIComponent(selected));
          if (version !== refreshVersion) return;
          const nextDetailKey = JSON.stringify([mission.id, mission.revision, mission.phase, mission.live, mission.stopping,
            mission.steps.map(step => step.evidenceState)]);
          if (nextDetailKey !== detailKey) { render(mission); detailKey = nextDetailKey; }
          detailLoadedAt = Date.now();
        }
      } else if (nextSelectionKey !== selectionKey) { detailKey = undefined; body.replaceChildren(muted(filter.value === "all"
        ? "Create a mission to turn an outcome into an ordered plan with persistent progress and automatic continuation."
        : "No missions match this filter.")); }
      selectionKey = nextSelectionKey;
      if (visible()) timer = setTimeout(refresh, live.some(row => ["Running", "Planning", "Queued"].includes(row.phase)) ? 3000 : 7000);
    } catch (error) {
      if (version === refreshVersion) {
        workflowStorageVersion = 0;
        create.disabled = improve.disabled = recover.disabled = true;
        overview.replaceChildren(muted("Workflow status unavailable. Displayed activity cannot be confirmed; retrying automatically."));
        overviewKey = undefined;
        detailKey = selectionKey = undefined;
        body.replaceChildren(muted(error.message), button("Retry", () => refresh(true)));
        if (visible()) timer = setTimeout(refresh, 15000);
      }
    }
  }
  const observer = new MutationObserver(() => {
    if (!visible()) { clearTimeout(timer); timer = undefined; }
    else if (!timer) void refresh();
  });
  const workbench = host.closest(".agent-workbench");
  if (workbench) observer.observe(workbench, { attributes: true, attributeFilter: ["hidden"] });
  document.addEventListener("visibilitychange", () => {
    clearTimeout(timer); timer = undefined;
    if (visible()) void refresh();
  });
  return { section, async create(source) {
    if (workflowStorageVersion !== 2) await refresh();
    if (workflowStorageVersion === 2) openCreate(source);
    else ctx.note("Can’t create a mission until the worker connects.", true);
  }, open(id) {
    if (id) { filter.value = "all"; selected = id; }
    void refresh();
    heading.focus({ preventScroll: true });
    heading.closest(".mission-panel").scrollIntoView({ block: "start" });
  } };
}
