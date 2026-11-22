// Global command palette (Ctrl+Shift+P or F1). Providers return commands
// lazily so lists such as files and catalog sections stay current.
// Command: { id, label, group, hint?, keys?, run() }.

const RECENT_KEY = "axiom-dev.palette.recent";
const providers = [];
let dialog, input, list, scope, items = [], cursor = 0, filterGroup = "";
let reportError = console.error;

const recent = {
  read() {
    try {
      const entries = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
      return Array.isArray(entries) ? entries : [];
    } catch { return []; }
  },
  push(id) {
    const next = [id, ...recent.read().filter(entry => entry !== id)].slice(0, 12);
    try { localStorage.setItem(RECENT_KEY, JSON.stringify(next)); } catch { /* private window */ }
  },
};

/** Register a function returning an array of commands. */
export function registerCommands(provider) { providers.push(provider); }

// Subsequence match with bonuses for word starts and contiguous runs.
export function fuzzyScore(query, text) {
  if (!query) return 1;
  const q = query.toLowerCase(), t = text.toLowerCase();
  const direct = t.indexOf(q);
  if (direct >= 0) return 1000 - direct + (direct === 0 || /[\s/·._-]/.test(t[direct - 1]) ? 200 : 0);
  let score = 0, last = -2, at = 0;
  for (const char of q) {
    if (char === " ") continue;
    const found = t.indexOf(char, at);
    if (found < 0) return 0;
    score += found === last + 1 ? 8 : 1;
    if (found === 0 || /[\s/·._-]/.test(t[found - 1])) score += 6;
    last = found;
    at = found + 1;
  }
  return score;
}

function collect() {
  const all = [];
  for (const provider of providers) {
    try { all.push(...(provider() ?? [])); } catch (error) { console.warn("palette provider failed", error); }
  }
  return all;
}

function render() {
  const raw = input.value.trim();
  // "group:" prefix narrows to one group, e.g. "file: recipes".
  const prefix = raw.match(/^(\w[\w ]*?):\s*(.*)$/);
  const query = prefix ? prefix[2] : raw;
  const groupQuery = prefix ? prefix[1].toLowerCase() : filterGroup.toLowerCase();
  const all = collect();
  const order = recent.read();
  const groups = [...new Set(all.map(command => command.group))];
  scope.replaceChildren(...["All", ...groups].map(name => {
    const active = (name === "All" && !groupQuery) || name.toLowerCase() === groupQuery;
    const chip = document.createElement("button");
    chip.type = "button";
    chip.textContent = name;
    chip.className = active ? "active" : "";
    chip.setAttribute("aria-pressed", String(active));
    chip.onclick = () => {
      filterGroup = name === "All" ? "" : name;
      if (prefix) input.value = query;
      cursor = 0;
      render();
      input.focus();
    };
    return chip;
  }));
  items = all
    .filter(command => !groupQuery || command.group.toLowerCase().startsWith(groupQuery))
    .map(command => {
      const score = fuzzyScore(query, command.label + " " + (command.hint ?? "") + " " + command.group);
      const recency = order.indexOf(command.id);
      // Navigation and actions outrank the long raw file list; command.weight overrides.
      const weight = score > 0 ? command.weight ?? (command.group === "File" ? 0 : 150) : 0;
      return { command, score: score && score + weight + (recency >= 0 ? (query ? 40 : 2000) - recency : 0) };
    })
    .filter(entry => entry.score > 0)
    .sort((a, b) => b.score - a.score || a.command.label.localeCompare(b.command.label))
    .slice(0, 80)
    .map(entry => entry.command);
  cursor = Math.min(cursor, Math.max(0, items.length - 1));
  list.replaceChildren(...items.map((command, index) => {
    const row = document.createElement("li");
    row.id = "palette-item-" + index;
    row.setAttribute("role", "option");
    row.setAttribute("aria-selected", String(index === cursor));
    row.className = "palette-item" + (index === cursor ? " active" : "");
    const label = document.createElement("span");
    label.className = "palette-label";
    label.textContent = command.label;
    const meta = document.createElement("span");
    meta.className = "palette-meta";
    if (order.includes(command.id) && !query) meta.append(Object.assign(document.createElement("em"), { textContent: "recent" }));
    if (command.hint) meta.append(Object.assign(document.createElement("small"), { textContent: command.hint }));
    if (command.keys) meta.append(Object.assign(document.createElement("kbd"), { textContent: command.keys }));
    meta.append(Object.assign(document.createElement("b"), { textContent: command.group }));
    row.append(label, meta);
    row.onmousemove = () => { if (cursor !== index) { cursor = index; mark(); } };
    row.onclick = () => execute(command);
    return row;
  }));
  list.scrollTop = 0;
  if (!items.length) {
    list.append(Object.assign(document.createElement("li"), {
      className: "palette-empty",
      role: "presentation",
      textContent: "No matches. Try a shorter search or choose All.",
    }));
  }
  mark();
}

function mark() {
  if (!items.length) {
    input.removeAttribute("aria-activedescendant");
    return;
  }
  [...list.children].forEach((row, index) => {
    row.classList.toggle("active", index === cursor);
    row.setAttribute("aria-selected", String(index === cursor));
  });
  list.children[cursor]?.scrollIntoView({ block: "nearest" });
  input.setAttribute("aria-activedescendant", items.length ? "palette-item-" + cursor : "");
}

async function execute(command) {
  dialog.close();
  try {
    await command.run();
    recent.push(command.id);
  } catch (error) {
    reportError(`${command.label} failed. ${error?.message ?? error}`);
  }
}

function build() {
  dialog = document.createElement("dialog");
  dialog.className = "palette";
  dialog.setAttribute("aria-label", "Command palette");
  input = document.createElement("input");
  input.type = "search";
  input.placeholder = "Type a command, file, section…  (group: narrows, e.g. file: recipes)";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-label", "Search commands");
  input.setAttribute("aria-controls", "palette-list");
  input.setAttribute("aria-expanded", "true");
  scope = document.createElement("div");
  scope.className = "qol-chips palette-scope";
  list = document.createElement("ul");
  list.id = "palette-list";
  list.setAttribute("role", "listbox");
  const foot = document.createElement("p");
  foot.className = "palette-foot";
  foot.textContent = "↑↓ move · Enter run · Tab reach filters · Esc close";
  dialog.append(input, scope, list, foot);
  document.body.append(dialog);
  input.addEventListener("input", () => { cursor = 0; render(); });
  input.addEventListener("keydown", event => {
    if (event.isComposing || !items.length) return;
    if (event.key === "ArrowDown") { event.preventDefault(); cursor = Math.min(items.length - 1, cursor + 1); mark(); }
    else if (event.key === "ArrowUp") { event.preventDefault(); cursor = Math.max(0, cursor - 1); mark(); }
    else if (event.key === "PageDown") { event.preventDefault(); cursor = Math.min(items.length - 1, cursor + 8); mark(); }
    else if (event.key === "PageUp") { event.preventDefault(); cursor = Math.max(0, cursor - 8); mark(); }
    else if (event.key === "Enter") { event.preventDefault(); if (items[cursor]) execute(items[cursor]); }
  });
  dialog.addEventListener("click", event => { if (event.target === dialog) dialog.close(); });
}

/** Open the palette, optionally pre-filtered to a group. */
export function openPalette(group = "") {
  if (!dialog) build();
  if (dialog.open) return;
  filterGroup = group;
  input.value = "";
  cursor = 0;
  render();
  dialog.showModal();
  input.focus();
}

/** Install the global shortcuts once. */
export function installPalette(onError = console.error) {
  reportError = onError;
  document.addEventListener("keydown", event => {
    const modifier = event.ctrlKey || event.metaKey;
    if (event.key === "F1" || (modifier && event.shiftKey && event.key.toLowerCase() === "p")) {
      if (document.querySelector("dialog[open]:not(.palette)")) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      openPalette();
    }
  }, true);
}
