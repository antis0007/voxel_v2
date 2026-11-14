// Shared section organizer for long stacked tool pages (Playtest, Foundry).
// Adds a sticky bar with text filter, category chips, jump menu and
// collapse/expand controls. Headings toggle their section. State persists
// per page in localStorage; every storage access tolerates private windows.

const store = {
  read(key) { try { return JSON.parse(localStorage.getItem(key) ?? "null"); } catch { return null; } },
  write(key, value) { try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* private window */ } },
};

const node = (tag, attrs = {}, ...children) => {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value == null) continue;
    if (key.startsWith("on")) element.addEventListener(key.slice(2), value);
    else if (key in element) element[key] = value;
    else element.setAttribute(key, value);
  }
  element.append(...children.filter(child => child != null));
  return element;
};

/**
 * @param {HTMLElement} host container whose sections are organized
 * @param {object} options
 * @param {string} options.key localStorage key for this page
 * @param {string} options.sections selector for organizable sections inside host
 * @param {string} [options.heading] selector for a section's heading (toggle target)
 * @param {Array<[string, RegExp]>} [options.categories] ordered [name, title pattern]; unmatched go to "Other"
 * @param {Element} [options.before] element the bar is inserted before (default: host.firstChild)
 * @param {boolean} [options.filter] false hides the text filter (page supplies its own)
 * @param {Element[]} [options.extra] controls appended to the bar
 * @returns {{ refresh(): void, entries(): Array<{title: string, category: string, reveal(): void}> }}
 */
export function organizePanels(host, options) {
  const heading = options.heading ?? "h2, h3, summary";
  const categories = options.categories ?? [];
  const saved = store.read(options.key) ?? {};
  const state = { query: "", category: saved.category ?? "All", collapsed: new Set(saved.collapsed ?? []) };
  const persist = () => store.write(options.key, { category: state.category, collapsed: [...state.collapsed] });

  const search = node("input", { type: "search", placeholder: "Filter sections (/)", "aria-label": "Filter sections" });
  const chips = node("div", { className: "qol-chips", role: "group", "aria-label": "Section categories" });
  const jump = node("select", { "aria-label": "Jump to section" });
  const count = node("span", { className: "qol-count", role: "status" });
  const collapseAll = node("button", { type: "button", title: "Collapse every section" }, "Collapse all");
  const expandAll = node("button", { type: "button", title: "Expand every section" }, "Expand all");
  const bar = node("div", { className: "qol-bar", role: "toolbar", "aria-label": "Section tools" },
    options.filter === false ? null : search, chips, jump, ...(options.extra ?? []),
    node("span", { className: "qol-spacer" }), count, collapseAll, expandAll);
  host.insertBefore(bar, options.before ?? host.firstChild);

  let sections = [];
  const titleOf = section => (section.querySelector(heading)?.textContent ?? "").trim();
  const categoryOf = title => categories.find(([, pattern]) => pattern.test(title))?.[0] ?? "Other";
  const setCollapsed = (entry, value) => {
    if (entry.section.tagName === "DETAILS") { entry.section.open = !value; return; }
    entry.section.classList.toggle("qol-collapsed", value);
    entry.toggle?.setAttribute("aria-expanded", String(!value));
    if (value) state.collapsed.add(entry.title); else state.collapsed.delete(entry.title);
  };

  function scan() {
    sections = [...host.querySelectorAll(options.sections)].filter(section => !bar.contains(section)).map(section => {
      const title = titleOf(section);
      const toggle = section.querySelector(heading);
      const entry = { section, title, category: categoryOf(title), toggle };
      if (toggle && toggle.tagName !== "SUMMARY" && !toggle.dataset.qol) {
        toggle.dataset.qol = "1";
        toggle.classList.add("qol-toggle");
        toggle.tabIndex = 0;
        toggle.setAttribute("role", "button");
        toggle.title = "Collapse or expand (click)";
        const flip = () => { setCollapsed(entry, !section.classList.contains("qol-collapsed")); persist(); };
        toggle.addEventListener("click", flip);
        toggle.addEventListener("keydown", event => {
          if (event.key === "Enter" || event.key === " ") { event.preventDefault(); flip(); }
        });
      }
      if (toggle?.tagName !== "SUMMARY") setCollapsed(entry, state.collapsed.has(title));
      return entry;
    }).filter(entry => entry.title);
  }

  function apply() {
    const words = state.query.toLowerCase().split(/\s+/).filter(Boolean);
    let shown = 0;
    for (const entry of sections) {
      const text = words.length ? entry.section.textContent.toLowerCase() : "";
      const visible = (state.category === "All" || entry.category === state.category)
        && words.every(word => text.includes(word));
      entry.section.classList.toggle("qol-filtered", !visible);
      // A text match opens a collapsed section so the hit is visible.
      if (visible && words.length) entry.section.classList.remove("qol-collapsed");
      else if (!words.length) entry.section.classList.toggle("qol-collapsed", state.collapsed.has(entry.title));
      if (visible) shown++;
    }
    count.textContent = shown === sections.length ? sections.length + " sections" : shown + " of " + sections.length;
    const names = ["All", ...new Set(sections.map(entry => entry.category))];
    if (!names.includes(state.category)) state.category = "All";
    chips.replaceChildren(...names.map(name => {
      const total = name === "All" ? sections.length : sections.filter(entry => entry.category === name).length;
      return node("button", { type: "button", className: state.category === name ? "active" : "",
        ariaPressed: String(state.category === name),
        onclick: () => { state.category = name; persist(); apply(); } }, name + " ", node("small", {}, String(total)));
    }));
    jump.replaceChildren(node("option", { value: "" }, "Jump to…"), ...sections
      .filter(entry => !entry.section.classList.contains("qol-filtered"))
      .map((entry, index) => node("option", { value: String(sections.indexOf(entry)) }, (index < 9 ? index + 1 + ". " : "") + entry.title)));
  }

  function reveal(entry) {
    if (entry.section.classList.contains("qol-filtered")) { state.query = search.value = ""; state.category = "All"; apply(); }
    setCollapsed(entry, false);
    persist();
    entry.section.scrollIntoView({ behavior: "smooth", block: "start" });
    entry.section.classList.add("qol-flash");
    setTimeout(() => entry.section.classList.remove("qol-flash"), 900);
  }

  search.addEventListener("input", () => { state.query = search.value; apply(); });
  search.addEventListener("keydown", event => {
    if (event.key === "Escape") { search.value = state.query = ""; apply(); }
    if (event.key === "Enter") {
      const first = sections.find(entry => !entry.section.classList.contains("qol-filtered"));
      if (first) reveal(first);
    }
  });
  jump.addEventListener("change", () => {
    const entry = sections[Number(jump.value)];
    jump.value = "";
    if (entry) reveal(entry);
  });
  collapseAll.onclick = () => { for (const entry of sections) setCollapsed(entry, true); persist(); };
  expandAll.onclick = () => { for (const entry of sections) setCollapsed(entry, false); persist(); };
  document.addEventListener("keydown", event => {
    if (options.filter === false || event.key !== "/" || host.offsetParent === null || event.target.matches("input,textarea,select,[contenteditable]")) return;
    event.preventDefault();
    search.focus();
    search.select();
  });

  const refresh = () => { scan(); apply(); };
  refresh();
  return {
    refresh,
    entries: () => sections.map(entry => ({ title: entry.title, category: entry.category, reveal: () => reveal(entry) })),
  };
}
