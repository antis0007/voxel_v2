import { el, button } from "./inspector.mjs";

const shortcuts = [
  ["Spells", "assets/spells.ron", "Forge Spark fixture and spell definitions"],
  ["Materials", "assets/materials.ron", "Blocks, surfaces, physics"],
  ["Recipes", "assets/recipes.ron", "Ingredients and outputs"],
  ["Creatures", "assets/bestiary.ron", "Health, movement, attacks"],
  ["Residents", "assets/fernwake_residents.ron", "Named identities, conversations and local records"],
  ["Item icons", "assets/icons/items16.txt", "16×16 pixels and palettes"],
  ["Tool masks", "assets/tools/sprites16.txt", "Part masks and material ramps"],
];

export function makeNavigation(api) {
  let entries = [];
  let pending = Promise.resolve();
  return {
    async load() {
      try { entries = await api("/api/navigation"); }
      catch { entries = []; }
    },
    get entries() { return entries; },
    get(path) { return entries.find(entry => entry.path === path); },
    remember(doc) {
      const { path, view, trail, spriteSettings } = doc;
      const sprite = spriteSettings && Object.fromEntries(
        ["entry", "ramp", "tileWidth", "tileHeight", "column", "row"].map(key => [key, spriteSettings[key]]));
      const current = { path, view, trail: [...trail], sprite };
      if (JSON.stringify(entries[0]) === JSON.stringify(current)) return;
      const snapshot = [current, ...entries.filter(entry => entry.path !== path)].slice(0, 8);
      entries = snapshot;
      pending = pending.then(() => api("/api/navigation", undefined, snapshot)).catch(() => {});
    },
  };
}

export function quickOpen(paths, entries, open, label, dirty) {
  const cards = shortcuts.filter(([, path]) => paths.has(path)).map(([name, path, description]) => {
    const card = button("", () => open(path), path, "start-card");
    card.append(el("strong", {}, name), el("span", {}, description));
    return card;
  });
  const recent = entries.filter(entry => paths.has(entry.path)).map(({ path }) => {
    const row = button("", () => open(path), path, "start-recent");
    row.append(el("strong", {}, label(path)), el("span", {}, dirty(path) ? "Unsaved" : path));
    return row;
  });
  return el("div", { className: "quick-open" },
    recent.length ? el("section", {}, el("h2", {}, "Recent files"), el("div", { className: "start-list" }, recent)) : null,
    el("section", {}, el("h2", {}, "Common files"), el("div", { className: "start-grid" }, cards)));
}
