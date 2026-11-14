import { el, button } from "./inspector.mjs";
import { field, textOf } from "./idea-data.mjs";
import { addEntry, patch } from "./syntax.mjs";
import { resolve } from "./targets.mjs";

const axes = [
  ["temperature", "Temperature", 0.5, 0.2, 0, 1],
  ["moisture", "Moisture", 0.5, 0.2, 0, 1],
  ["elevation", "Elevation (m)", 60, 30, -512, 2000],
  ["slope", "Slope (rise/run)", 0.25, 0.2, 0, 10],
  ["wetness", "Water proximity", 0.5, 0.2, 0, 1],
];
export const biomeColors = { Meadow: "#a5be64", Forest: "#54895b", Taiga: "#418785", Tundra: "#a1bcc6",
  Alpine: "#c8dae9", Desert: "#e0ae5c", Badlands: "#c47550", Savanna: "#d7c17a", Rainforest: "#378a6b",
  Heathland: "#967ea8", Fen: "#709e8c", VolcanicField: "#bd6852", KarstUpland: "#8592bb" };
const human = name => name.replace(/([a-z])([A-Z])/g, "$1 $2");
const option = (value, name) => el("option", { value }, name);
const hint = text => el("p", { className: "muted" }, text);
const label = (name, input) => el("label", {}, name, input);
const input = (value, min, max, step = "any") => el("input", { type: "number", value: String(value), min, max, step, required: true });
const decimal = value => Number.isInteger(value) ? value + ".0" : String(value);

function config(row) {
  const result = {
    name: textOf(field(row, "name")), bias: Number(textOf(field(row, "bias"))) || 0,
    gate: textOf(field(row, "requires")?.children?.[0]),
  };
  for (const [key] of axes) {
    const node = field(row, key);
    const fit = node?.prefix === "Some" ? node.children[0] : node;
    const centre = field(fit, "centre"), width = field(fit, "width");
    if (centre && width) result[key] = { centre: Number(textOf(centre)), width: Number(textOf(width)),
      open: textOf(field(fit, "open")) || "Neither" };
  }
  return result;
}

function score(family, sample) {
  const gate = family.gate ? sample[family.gate] : 1;
  if (!(gate > 0)) return -Infinity;
  let score = family.bias + Math.log(gate);
  for (const [key] of axes) {
    const axis = family[key];
    if (!axis) continue;
    let distance = (sample[key] - axis.centre) / axis.width;
    if (axis.open === "Above") distance = Math.min(0, distance);
    if (axis.open === "Below") distance = Math.max(0, distance);
    score -= distance * distance;
  }
  return score;
}

export function biomeStudio(ctx, doc, spec, changed) {
  let original = doc.text;
  const root = ctx.tree(doc);
  const found = resolve(root, spec.parts);
  if (!found) throw new Error("This biome moved. Reopen it from the catalog.");
  const table = field(root, "families").children;
  const index = table.indexOf(found.node), families = table.map(config);
  let current = families[index];
  const sample = { temperature: current.temperature.centre, moisture: current.moisture.centre,
    elevation: current.elevation?.centre ?? 60, slope: current.slope?.centre ?? 0.15,
    wetness: current.wetness?.centre ?? 0.3, Volcanic: 0, Carbonate: 0, Anomaly: 0 };
  if (current.gate) sample[current.gate] = 1;
  const controls = axes.map(([key, title, centre, width, min, max], i) => {
    const axis = current[key];
    const target = input(axis?.centre ?? centre, min, max);
    const range = input(axis?.width ?? width, 0.0001, max - min);
    const open = el("select", {}, option("Neither", "Both sides"), option("Above", "No upper penalty"),
      option("Below", "No lower penalty"));
    open.value = axis?.open ?? "Neither";
    const enabled = el("input", { type: "checkbox", checked: Boolean(axis) });
    const slider = i < 2 ? el("input", { type: "range", min, max, step: 0.01, value: target.value,
      "aria-label": title + " target slider" }) : null;
    target.setAttribute("aria-label", title + " best at");
    range.setAttribute("aria-label", title + " tolerance");
    if (slider) {
      slider.oninput = () => { target.value = slider.value; };
      target.oninput = () => { if (target.value && target.validity.valid) slider.value = target.value; };
    }
    const fields = el("div", { className: "biome-axis-fields" }, label("Best at", target), label("Tolerance", range), label("Curve", open));
    function toggle() {
      const disabled = i > 1 && !enabled.checked;
      fields.hidden = disabled;
      target.disabled = range.disabled = open.disabled = disabled;
    }
    enabled.onchange = toggle;
    toggle();
    return { key, target, range, open, enabled, view: el("div", { className: "biome-axis" },
      i < 2 ? el("strong", {}, title) : el("label", { className: "biome-toggle" }, enabled, title), slider, fields) };
  });
  const bias = input(current.bias, -10, 10);
  const gate = el("select", {}, option("", "Any rock"), option("Volcanic", "Volcanic"), option("Carbonate", "Carbonate"), option("Anomaly", "Anomaly"));
  gate.value = current.gate;
  const climateActions = el("div", { className: "world-actions biome-save" });
  const changes = el("div", { className: "biome-controls" }, el("section", { className: "world-card" },
    el("h3", {}, "Climate fit"), controls.slice(0, 2).map(control => control.view), climateActions),
  el("details", { className: "world-card" }, el("summary", {}, "Terrain and rock rules"),
    controls.slice(2).map(control => control.view),
    el("div", { className: "biome-axis-fields" }, label("Rock required", gate), label("Preference boost", bias))));
  function settings(strict = false) {
    const draft = { name: current.name, gate: gate.value };
    const value = (control, fallback) => {
      if (control.value && control.validity.valid) return Number(control.value);
      if (strict) throw new Error("Enter a value within range for each active biome setting.");
      return fallback;
    };
    draft.bias = value(bias, current.bias);
    controls.forEach(({ key, target, range, open, enabled }, i) => {
      if (i < 2 || enabled.checked) draft[key] = { centre: value(target, current[key]?.centre ?? axes[i][2]),
        width: value(range, current[key]?.width ?? axes[i][3]), open: open.value };
    });
    return draft;
  }
  const map = el("canvas", { width: 112, height: 74, "aria-label": "Biome winners by temperature and moisture" });
  const mapWrap = el("div", { className: "biome-map" }, map);
  const legend = el("div", { className: "biome-legend" });
  const result = el("div", { className: "biome-winner", role: "status" });
  const sampleFields = [["temperature","Temperature",0,1,.01], ["moisture","Moisture",0,1,.01],
    ["elevation","Elevation (m)",-512,2000,1], ["slope","Slope",0,10,.01], ["wetness","Water proximity",0,1,.01],
    ["Volcanic","Volcanic rock",0,1,.01], ["Carbonate","Carbonate rock",0,1,.01], ["Anomaly","Anomaly",0,1,.01]]
    .map(([key, title, min, max, step]) => {
      const control = input(sample[key], min, max, step);
      control.oninput = () => {
        if (control.value && control.validity.valid) { sample[key] = Number(control.value); queueDraw(); }
      };
      return { key, control, view: label(title, control) };
    });
  const sampleSettings = el("details", { className: "biome-sample" }, el("summary", {}, "Sample conditions"),
    el("div", { className: "biome-axis-fields" }, sampleFields.map(item => item.view)));
  let frame;
  function queueDraw() { frame ??= requestAnimationFrame(draw); }
  function draw() {
    frame = undefined;
    const draft = settings();
    const fitted = families.map((family, i) => i === index ? draft : family);
    const colors = fitted.map(family => biomeColors[family.name] ?? "#8e9aa6");
    const context = map.getContext("2d");
    const used = new Set();
    const point = { ...sample };
    for (let y = 0; y < map.height; y++) {
      point.moisture = 1 - y / (map.height - 1);
      for (let x = 0; x < map.width; x++) {
        point.temperature = x / (map.width - 1);
        let best = -Infinity, winner = 0;
        fitted.forEach((family, i) => {
          const rank = score(family, point);
          if (rank > best) { best = rank; winner = i; }
        });
        used.add(winner);
        context.fillStyle = colors[winner];
        context.fillRect(x, y, 1, 1);
      }
    }
    const ranked = fitted.map((family, i) => [score(family, sample), i]).sort((a, b) => b[0] - a[0]);
    const [first, second] = ranked;
    const runner = 100 / (1 + Math.exp(first[0] - second[0]));
    const share = Number.isFinite(runner) ? Math.round(runner) : 0;
    const x = sample.temperature * (map.width - 1), y = (1 - sample.moisture) * (map.height - 1);
    context.fillStyle = "#142027";
    context.fillRect(x - 3, y - 3, 7, 7);
    context.strokeStyle = "#fff4de";
    context.strokeRect(x - 2.5, y - 2.5, 6, 6);
    legend.replaceChildren(...[...used].sort((a, b) => a - b).map(i => el("span", { className: i === index ? "selected" : "" },
      el("i", { style: "background:" + colors[i] }), human(fitted[i].name))));
    result.replaceChildren(el("strong", {}, human(fitted[first[1]].name)),
      el("span", {}, share ? (100 - share) + "% · " + human(fitted[second[1]].name) + " " + share + "%" : "Wins this sample"));
  }
  map.onclick = event => {
    const bounds = map.getBoundingClientRect();
    sample.temperature = Math.min(1, Math.max(0, (event.clientX - bounds.left) / bounds.width));
    sample.moisture = Math.min(1, Math.max(0, 1 - (event.clientY - bounds.top) / bounds.height));
    for (const { key, control } of sampleFields.slice(0, 2)) control.value = sample[key].toFixed(2);
    queueDraw();
  };
  changes.addEventListener("input", queueDraw);
  changes.addEventListener("change", queueDraw);
  const apply = button("Review changes", () => {
    try {
      if (doc.text !== original) throw new Error("This file changed. Reopen the biome before applying.");
      const draft = settings(true);
      const updates = [["bias", decimal(draft.bias)], ["requires", draft.gate ? "Some(" + draft.gate + ")" : "None"]];
      for (const [key] of axes) {
        const a = draft[key], old = current[key];
        if (JSON.stringify(a) === JSON.stringify(old)) continue;
        const fit = a ? "(centre: " + decimal(a.centre) + ", width: " + decimal(a.width) +
          (a.open === "Neither" ? "" : ", open: " + a.open) + ")" : "";
        updates.push([key, ["temperature","moisture"].includes(key) ? fit : a ? "Some(" + fit + ")" : "None"]);
      }
      let text = doc.text;
      for (const [key, raw] of updates) {
        if ((key === "bias" && draft.bias === current.bias) || (key === "requires" && draft.gate === current.gate)) continue;
        const row = resolve(ctx.parse(text, "ron"), spec.parts).node;
        const node = field(row, key);
        text = node ? patch(text, node.start, node.end, raw) : addEntry(text, row, key + ": " + raw);
      }
      if (text === original) {
        if (ctx.dirty(doc)) ctx.review(doc);
        else ctx.note("No biome changes to review");
        return;
      }
      if (ctx.edit(doc, text)) {
        original = doc.text;
        current = draft;
        changed();
        ctx.review(doc);
      }
    } catch (error) { ctx.note(error.message, true); }
  }, "See the biome edits before saving", "primary");
  climateActions.append(apply);
  draw();
  return el("div", { className: "biome-studio" }, el("section", { className: "world-card biome-preview" },
    el("div", { className: "world-card-head" }, el("h3", {}, "Climate map"), hint("Click to sample · wet at the top")),
    mapWrap, el("div", { className: "biome-map-axis" }, el("span", {}, "Cool"), el("span", {}, "Warm")),
    result, legend, sampleSettings), changes);
}
