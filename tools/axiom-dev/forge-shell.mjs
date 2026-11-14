// Shared app shell for every dev tool page: app switcher, live engine and AI
// status, command palette button, and the cross-page "send to AI" handoff.
import { openPalette } from "./command-palette.mjs";

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
  const response = await fetch(route, { headers: { "X-Axiom-Session": token } });
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
  async function poll() {
    if (document.hidden) return;
    try {
      const game = await get("/api/game");
      const state = game.building ? ["busy", "Building"] : game.ready ? ["live", "World ready"] : game.running ? ["busy", "Loading"] : ["idle", "No world"];
      engine.dataset.state = state[0];
      engine.lastChild.textContent = state[1];
      engine.title = "Playtest engine · " + state[1] + (game.run?.id ? " · run " + game.run.id : "");
    } catch { engine.dataset.state = "down"; engine.lastChild.textContent = "Offline"; }
    try {
      const run = await get("/api/agent");
      const phase = run.phase ?? "Idle";
      ai.dataset.state = phase === "Idle" ? "idle" : /error|fail/i.test(phase) ? "down" : phase === "Ready" ? "live" : "busy";
      ai.lastChild.textContent = phase === "Idle" ? "AI idle" : "AI " + phase.toLowerCase();
    } catch { ai.dataset.state = "down"; ai.lastChild.textContent = "AI offline"; }
  }
  poll();
  setInterval(poll, 3000);
  document.addEventListener("visibilitychange", poll);
  return [engine, ai];
}

/**
 * Mount the shell. `host` is an existing header to fill (main panel) or
 * null to prepend a slim bar (standalone pages).
 */
export function mountShell({ host = null, current = location.pathname } = {}) {
  const here = current === "/index.html" ? "/" : current;
  const apps = node("nav", { className: "forge-apps", ariaLabel: "Dev tools" }, ...APPS.map(([path, label, title]) =>
    node("a", { href: path + location.hash, title, className: path === here ? "active" : "",
      ariaCurrent: path === here ? "page" : null }, label)));
  const palette = node("button", { type: "button", className: "forge-palette", title: "Command palette (Ctrl+Shift+P or F1)",
    onclick: () => openPalette() }, "Commands ", node("kbd", {}, "F1"));
  const status = node("div", { className: "forge-status" }, ...statusChips(), palette);
  if (host) { host.append(apps, status); return; }
  const bar = node("header", { className: "forge-shell" },
    node("a", { className: "forge-brand", href: "/" + location.hash, title: "Axiom Dev Panel" }, node("span", { className: "forge-mark" }, "A"), "Axiom"),
    apps, status);
  document.body.prepend(bar);
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
    if (!acknowledged) window.open("/" + location.hash, "axiom-panel");
    resolve(acknowledged);
  }, 450));
}

/** Hand a task (and up to four source paths) to the AI workbench. */
export const sendToAi = (task, paths = []) => sendToPanel({ type: "assist", task, paths: paths.slice(0, 4) });
/** Open a project file in the panel's Workspace. */
export const openInPanel = (path, line) => sendToPanel({ type: "open", path, line });

/** Main panel side: receive messages live and pick up one queued before load. */
export function receivePanel(handlers) {
  const take = message => {
    if (!message?.at || !handlers[message.type] || Date.now() - message.at > 120_000) return;
    try { localStorage.removeItem(PENDING); } catch { /* private window */ }
    channel?.postMessage({ type: "ack", at: message.at });
    handlers[message.type](message);
  };
  channel?.addEventListener("message", event => take(event.data));
  try { take(JSON.parse(localStorage.getItem(PENDING) ?? "null")); } catch { /* malformed queue */ }
}
