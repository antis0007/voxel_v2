
import "./app-updates.mjs";
// Shared app shell for every dev tool page: app switcher, live engine and AI
// status, command palette button, and the cross-page "send to AI" handoff.
import { openPalette, registerCommands } from "./command-palette.mjs";

const token = location.hash.slice(1);
const APPS = [
  ["/", "Panel", "Catalog, graph, files, AI and playtest"],
  ["/parity", "Parity", "Mod feature ledger and AI work queue"],
  ["/wand-lab", "Wand Lab", "Deck designer scored by the native evaluator"],
  ["/foundry", "Foundry", "Native captures and research evidence"],
];
export const channel = typeof BroadcastChannel === "function" ? new BroadcastChannel("axiom-forge") : null;
const PENDING = "axiom-forge.pending-assist";

async function get(route) {
  const response = await fetch(route, { headers: { "X-Axiom-Session": token }, signal: AbortSignal.timeout(4500) });
  if (!response.ok) throw new Error(response.statusText);
  return response.json();
}

const node = (tag, props = {}, ...kids) => {
  const element = Object.assign(document.createElement(tag), props);
  element.append(...kids.filter(kid => kid != null));
  return element;
};

/** Live status chips; polls only while the page is visible. */
function statusChips() {
  const engine = node("span", { className: "forge-chip", title: "Playtest engine" }, node("i"), node("span", {}, "Engine"));
  const ai = node("span", { className: "forge-chip", title: "AI workbench run" }, node("i"), node("span", {}, "AI"));
  function update({ work, run, game } = {}) {
    if (game) {
      const state = game.building ? ["busy", "Building"] : game.ready ? ["live", "World ready"] : game.running ? ["busy", "Loading"] : ["idle", "No world"];
      engine.dataset.state = state[0];
      engine.lastChild.textContent = state[1];
      engine.title = "Playtest engine · " + state[1] + (game.run?.id ? " · run " + game.run.id : "");
    } else { engine.dataset.state = "down"; engine.lastChild.textContent = "Offline"; }
    if (work && run) {
      const running = work.missions.find(mission => !mission.readOnly && ["Running", "Planning"].includes(mission.phase));
      const queued = work.missions.filter(mission => !mission.readOnly && mission.phase === "Queued").length;
      const phase = running?.phase ?? run.phase ?? "Idle";
      ai.dataset.state = running || queued ? "busy" : phase === "Idle" ? "idle" : /error|fail/i.test(phase) ? "down" : "live";
      ai.lastChild.textContent = running ? "AI working · " + queued + " queued" : queued ? "AI queue · " + queued : phase === "Idle" ? "AI idle" : "AI " + phase.toLowerCase();
      ai.title = running?.objective ?? (work.autonomy?.enabled ? "Autonomous board intake enabled" : "AI workbench");
    } else { ai.dataset.state = "down"; ai.lastChild.textContent = "AI offline"; }
  }
  return { engine, ai, update };
}

/**
 * Mount the shell. `host` is an existing header to fill (main panel) or
 * null to prepend a slim bar (standalone pages).
 */
export function mountShell({ host = null, current = location.pathname, context, openTask, showWorkbench } = {}) {
  const here = current === "/index.html" ? "/" : current;
  const tool = APPS.find(([path]) => path === here)?.[1] ?? "Dev tools";
  const ask = node("button", { type: "button", className: "forge-ask", onclick: () => composer.show() },
    node("span", { ariaHidden: "true" }, "✦"), "Ask AI");
  const composer = assistComposer(() => context?.() ?? { label: tool }, openTask);
  registerCommands(() => [
    { id: "forge:ask", label: "Ask AI about this view", group: "AI", run: () => composer.show() },
    { id: "forge:ai", label: "Open AI workbench", group: "AI", run: showWorkbench ?? openAiWorkbench },
  ]);
  const apps = node("nav", { className: "forge-apps", ariaLabel: "Dev tools" }, ...APPS.map(([path, label, title]) =>
    node("a", { href: path + location.hash, title, className: path === here ? "active" : "",
      ariaCurrent: path === here ? "page" : null }, label)));
  const palette = node("button", { type: "button", className: "forge-palette", title: "Command palette (Ctrl+Shift+P or F1)",
    onclick: () => openPalette() }, "Commands ", node("kbd", {}, "F1"));
  const chips = statusChips();
  const status = node("div", { className: "forge-status" }, chips.engine, chips.ai,
    workDock(showWorkbench ?? openAiWorkbench, chips.update), ask, palette);
  if (host) { host.append(apps, status); return; }
  const bar = node("header", { className: "forge-shell" },
    node("a", { className: "forge-brand", href: "/" + location.hash, title: "Axiom Dev Panel" }, node("span", { className: "forge-mark" }, "A"), "Axiom"),
    apps, status);
  document.body.prepend(bar);
}

function assistComposer(readContext, openTask) {
  let selection;
  const title = node("h2", { id: "forge-assist-title" }, "Ask AI");
  const context = node("div", { className: "forge-assist-context" });
  const task = node("textarea", {
    rows: 4, maxLength: 1600, required: true, ariaLabel: "What do you want to do?",
    placeholder: "Describe the outcome, question, or suspected problem…",
  });
  const modes = node("select", { ariaLabel: "Task type" },
    node("option", { value: "refine" }, "Propose changes"),
    node("option", { value: "audit" }, "Investigate a problem"),
    node("option", { value: "systems" }, "Review a system"));
  const form = node("form", {},
    title, context, node("label", {}, "Task", task),
    node("div", { className: "forge-assist-actions" }, modes,
      node("button", { type: "button", onclick: () => dialog.close() }, "Cancel"),
      node("button", { type: "submit", className: "forge-ask" }, "Open in AI workbench")));
  const dialog = node("dialog", { className: "forge-assist" }, form);
  dialog.setAttribute("aria-labelledby", title.id);
  document.body.append(dialog);
  form.onsubmit = event => {
    event.preventDefault();
    const text = task.value.trim();
    if (!text) { task.focus(); return; }
    const payload = {
      type: "assist",
      task: text + (selection.summary ? "\n\nSelected context:\n" + selection.summary.slice(0, 660) : ""),
      sources: (selection.sources ?? []).slice(0, 5).map(({ path, label, parts, line, name, spriteId, valueOnly }) =>
        ({ path, label, parts, line, name, spriteId, valueOnly })),
      mode: modes.value,
      origin: selection.label,
      parityId: selection.parityId,
    };
    dialog.close();
    if (openTask) openTask(payload);
    else void sendToPanel(payload);
  };
  return {
    show() {
      selection = readContext() ?? { label: "Current tool" };
      const sources = (selection.sources ?? []).map(source => source.label ?? source.path);
      if (!sources.length) modes.value = "audit";
      context.replaceChildren(node("strong", {}, selection.label),
        ...(sources.length ? [node("small", {}, sources.join(" · "))] : []),
        ...(selection.summary ? [node("small", {}, selection.summary)] : []));
      if (!dialog.open) dialog.showModal();
      task.focus();
    },
  };
}

/**
 * Send a message to the main panel, opening it when no panel acknowledges.
 * The message is also queued so a panel that is still loading picks it up.
 */
function sendToPanel(message) {
  message.at = Date.now();
  try { localStorage.setItem(PENDING, JSON.stringify(message)); } catch { /* private window */ }
  let acknowledged = false;
  const listen = event => { if (event.data?.type === "ack" && event.data.at === message.at) acknowledged = true; };
  channel?.addEventListener("message", listen);
  channel?.postMessage(message);
  return new Promise(resolve => setTimeout(() => {
    channel?.removeEventListener("message", listen);
    if (!acknowledged && !window.open("/" + location.hash, "axiom-panel")) location.assign("/" + location.hash);
    resolve(acknowledged);
  }, 450));
}

/** Hand a task (and up to four source paths) to the AI workbench. */
export const sendToAi = (task, paths = [], { mode, origin, parityId, createMission } = {}) =>
  sendToPanel({ type: "assist", task, paths: paths.slice(0, 4), mode, origin, parityId, createMission });
export const openAiWorkbench = missionId => sendToPanel({ type: "show-ai", missionId: typeof missionId === "string" ? missionId : undefined });
/** Open a project file in the panel's Workspace. */
export const openInPanel = (path, line) => sendToPanel({ type: "open", path, line });

/** Main panel side: receive messages live and pick up one queued before load. */
export function receivePanel(handlers) {
  const take = message => {
    if (!message?.at || !Object.hasOwn(handlers, message.type) || Date.now() - message.at > 120_000) return;
    try { localStorage.removeItem(PENDING); } catch { /* private window */ }
    channel?.postMessage({ type: "ack", at: message.at });
    handlers[message.type](message);
  };
  channel?.addEventListener("message", event => take(event.data));
  try { take(JSON.parse(localStorage.getItem(PENDING) ?? "null")); } catch { /* malformed queue */ }
}

function workDock(openWorkbench, updateChips) {
  const toggle = node('button', { type:'button', className:'forge-work-toggle' }, 'Live work');
  const freshness = node('small', {}, 'Connecting…');
  const content = node('div', { className:'work-dock-content' });
  const search = node('input', { type:'search', placeholder:'Find a task or agent…', ariaLabel:'Find a task or agent' });
  const filters = node('div', { className:'work-filters', ariaLabel:'Filter live tasks' });
  const counts = node('div', { className:'work-counts' });
  const expandedIds = new Set();
  let filter = 'all', snapshot;
  const filterButtons = ['all','active','waiting','queued','finished'].map(value => {
    const button = node('button', { type:'button', onclick:() => { filter = value; content.scrollTop = 0; render(); } },
      ({all:'All',active:'Active',waiting:'Waiting',queued:'Queued',finished:'Finished'})[value]);
    button.dataset.filter = value; filters.append(button); return button;
  });
  search.oninput = () => { content.scrollTop = 0; render(); };
  const resize = node('div', { className:'work-dock-resize', role:'separator', tabIndex:0, ariaLabel:'Live work width', ariaOrientation:'vertical' });
  function setWidth(value) {
    const width = Math.round(Math.max(300, Math.min(520, innerWidth - 720, value)));
    document.body.style.setProperty('--work-dock-width', width + 'px');
    resize.setAttribute('aria-valuenow', String(width));
    resize.setAttribute('aria-valuemin', '300');
    resize.setAttribute('aria-valuemax', String(Math.max(300, Math.min(520, innerWidth - 720))));
    try { localStorage.setItem('axiom.live-work-width', String(width)); } catch {}
  }
  let width = 360;
  try { width = Number(localStorage.getItem('axiom.live-work-width')) || width; } catch {}
  setWidth(width);
  resize.onpointerdown = event => { if (event.button !== 0) return; resize.setPointerCapture(event.pointerId); event.preventDefault(); };
  resize.onpointermove = event => { if (resize.hasPointerCapture(event.pointerId)) setWidth(innerWidth - event.clientX); };
  resize.onpointerup = event => { if (resize.hasPointerCapture(event.pointerId)) resize.releasePointerCapture(event.pointerId); };
  resize.onkeydown = event => {
    if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    event.preventDefault();
    const current = Number(resize.getAttribute('aria-valuenow'));
    setWidth(event.key === 'Home' ? 300 : event.key === 'End' ? 520 : current + (event.key === 'ArrowLeft' ? 20 : -20));
  };
  window.addEventListener('resize', () => setWidth(Number(resize.getAttribute('aria-valuenow'))));
  const dock = node('aside', { className:'work-dock', ariaLabel:'Live agents and tasks', id:'live-work-dock' },
    node('header', {}, node('h2', {}, 'Live work'), node('button', { type:'button', ariaLabel:'Close live work', onclick:() => setOpen(false) }, '×')),
    resize, freshness, counts, node('div', {className:'work-tools'}, search, filters), content, node('footer', {}, node('button', { type:'button', onclick:openWorkbench }, 'Open AI workspace'),
      node('small', {}, 'Tracks this dev worker. External chats are not connected.')));
  document.body.append(dock);
  toggle.setAttribute('aria-controls', dock.id);
  function setOpen(value) {
    dock.hidden = !value;
    toggle.setAttribute('aria-expanded', String(value));
    document.body.classList.toggle('work-dock-open', value);
    try { localStorage.setItem('axiom.live-work', String(value)); } catch {}
    if (!value && dock.contains(document.activeElement)) toggle.focus();
    if (value && snapshot) render();
  }
  let saved;
  try { saved = localStorage.getItem('axiom.live-work'); } catch {}
  setOpen(saved == null ? innerWidth >= 1500 : saved === 'true');
  toggle.onclick = () => setOpen(dock.hidden);
  dock.addEventListener('keydown', event => { if (event.key === 'Escape') setOpen(false); });
  let pending = false, signature;
  content.addEventListener('toggle', event => {
    const item = event.target;
    if (item.matches('.work-task') && item.isConnected) { if (item.open) expandedIds.add(item.dataset.id); else expandedIds.delete(item.dataset.id); }
  }, true);
  const active = phase => ['Running','Planning'].includes(phase);
  const finished = phase => ['Complete','Cancelled','Archived'].includes(phase);
  function card(mission) {
    const progress = mission.progress, last = mission.lastEvent ?? mission.events?.at(-1);
    const step = mission.live?.step ?? mission.nextStep ?? mission.steps?.find(row => !['Saved','Accepted','Reported'].includes(row.status))?.title;
    const details = node('details', { className:'work-task' });
    details.append(node('summary', {}, node('span', { className:'work-phase' }, mission.phase),
      node('strong', {}, (mission.objective ?? 'Task').split('\n')[0]),
      node('small', {}, [mission.settings?.provider,step].filter(Boolean).join(' · ') || 'Waiting for its next step')));
    if (progress?.total) details.querySelector('summary').append(node('small', {}, progress.finished + ' / ' + progress.total + ' steps complete'));
    if (['Blocked','Deferred','Interrupted'].includes(mission.phase)) details.querySelector('summary').append(node('small', { className:'work-error' }, mission.error || last?.detail || 'Waiting for recovery'));
    if (progress?.total) details.append(node('progress', { max:progress.total, value:progress.finished, ariaLabel:'Completed steps' }), node('small', {}, `${progress.finished} of ${progress.total} steps complete`));
    if (mission.error) details.append(node('p', { className:'work-error' }, mission.error));
    if (last) details.append(node('p', {}, last.detail), node('small', {}, 'Last event · ' + last.at));
    details.append(node('p', {}, mission.objective));
    if (mission.paths?.length) details.append(node('small', {className:'work-paths'}, mission.paths.join(' · ')));
    details.append(node('button', {type:'button', className:'work-open-mission', onclick:() => {
      if (location.pathname === '/' || location.pathname === '/index.html') {
        window.dispatchEvent(new CustomEvent('axiom:open-mission', {detail:mission.id}));
      } else openAiWorkbench(mission.id);
      if (innerWidth < 1100) setOpen(false);
    }}, 'Open task →'));
    details.dataset.phase = mission.phase;
    return details;
  }
  function render() {
    if (!snapshot) return;
    const {work,run,game} = snapshot;
    const missions = (work.missions ?? []).filter(row => !row.readOnly && !row.legacy);
    const running = missions.filter(row => active(row.phase));
    const queued = missions.filter(row => row.phase === 'Queued');
    const waiting = missions.filter(row => !active(row.phase) && row.phase !== 'Queued' && !finished(row.phase));
    toggle.textContent = 'Live work · ' + running.length + ' active · ' + queued.length + ' queued';
    if (dock.hidden) return;
    filterButtons.forEach(button => button.setAttribute('aria-pressed', String(button.dataset.filter === filter)));
    const query = search.value.trim().toLowerCase();
    const key = JSON.stringify([missions.map(({elapsedMs,usage,...row}) => row),work.lane,run.phase,run.step,game.building,game.ready,game.running,filter,query]);
    if (key === signature) return;
    counts.replaceChildren(...[[running.length,'Active'],[queued.length,'Queued'],[waiting.length,'Waiting']].map(([count,label]) => node('span', {}, node('strong', {}, String(count)), label)));
    const focused = document.activeElement?.closest('.work-task');
    const focusId = focused?.dataset.id;
    const focusAction = document.activeElement?.classList.contains('work-open-mission');
    const scroll = content.scrollTop;
    const nodes = [node('p', {className:'work-lane'}, work.lane ? (work.lane.alive ? 'Agent working · ' : 'Worker interrupted · ') + (work.lane.label ?? 'AI lane') : 'Agent lane free · ' + (run.phase ?? 'Idle')),
      node('small', {}, game.building ? 'Playtest · building' : game.ready ? 'Playtest · world ready' : game.running ? 'Playtest · loading' : 'Playtest · no world running')];
    let matches = 0;
    for (const [id,label,rows] of [['active','Working now',running],['waiting','Waiting / needs attention',waiting],['queued','Up next',queued],['finished','Recently finished',missions.filter(row => finished(row.phase))]]) {
      if (filter !== 'all' && filter !== id) continue;
      const matching = rows.filter(row => !query || [row.objective,row.settings?.provider,row.phase,...(row.paths ?? [])].join(' ').toLowerCase().includes(query));
      const visible = id === 'finished' && filter === 'all' && !query ? matching.slice(0,5) : matching;
      if (!visible.length) continue;
      matches += visible.length;
      nodes.push(node('h3', {}, label + ' · ' + matching.length));
      for (const row of visible) { const item = card(row); item.dataset.id = row.id; item.open = expandedIds.has(row.id); nodes.push(item); }
    }
    if (!matches) nodes.push(node('p', {className:'work-empty'}, query ? 'No matching tasks. Try another name, provider, or file.' : 'No tasks in this view. Work will appear here as its status changes.'));
    content.replaceChildren(...nodes);
    if (focusId) {
      const restored = [...content.querySelectorAll('.work-task')].find(item => item.dataset.id === focusId);
      restored?.querySelector(focusAction ? '.work-open-mission' : 'summary')?.focus({preventScroll:true});
    }
    content.scrollTop = scroll; signature = key;
  }
  async function refresh() {
    if (pending || document.hidden) return;
    pending = true;
    try {
      const [work, run, game] = (await Promise.allSettled(['/api/workflows?view=summary','/api/agent','/api/game'].map(get)))
        .map(result => result.status === 'fulfilled' ? result.value : undefined);
      updateChips({work,run,game});
      if (!work || !run || !game) throw new Error('Live status unavailable');
      snapshot = {work,run,game};
      freshness.textContent = 'Live · updated ' + new Date().toLocaleTimeString();
      dock.dataset.connection = 'live';
      render();
    } catch { dock.dataset.connection = 'stale'; freshness.textContent = 'Connection lost · last known state · retrying'; toggle.textContent = 'Live work · reconnecting'; }
    finally { pending = false; }
  }
  refresh(); setInterval(refresh,3000); document.addEventListener('visibilitychange',refresh);
  return toggle;
}
