import { el, button } from "./inspector.mjs";
import { patch } from "./syntax.mjs";

export const depositPath = "crates/worldgen/src/ore_geology.rs";
const depth = ["Starts below surface (m)", /below < (\d+\.\d+)/, 1.5, 240];
export const deposits = [
  { key: "coal_seam", label: "Coal", form: "Seams", color: "#6e8196", items: ["Resource(3)", "Block(CoalOre)"],
    description: "Thin seams in young, quiet continental basins.",
    values: [["Starts below surface (m)", /\!\((\d+\.\d+)\.\./, 1.5, 240],
      ["Stops below surface (m)", /\.\.(\d+\.\d+)\)\.contains/, 2, 512],
      ["Seam spacing (m)", /PERIOD: f32 = (\d+\.\d+)/, 2, 128],
      ["Basin cutoff", /basin - (\d+\.\d+)/, 0, 1]] },
  { key: "iron_bands", label: "Iron", form: "Bands", color: "#da9076", items: ["Resource(2)", "Block(IronOre)", "Block(Hematite)", "Block(Magnetite)"],
    description: "Banded formations in older rock. Copper stocks can also have iron halos.",
    values: [depth, ["Minimum crust age", /crust_age < (\d+\.\d+)/, 0, 1],
      ["Band spacing (m)", /bed \/ (\d+\.\d+)/, 1, 128],
      ["Band thickness fraction", /phase > (\d+\.\d+)/, 0.005, 0.5]] },
  { key: "copper_stock", label: "Copper", form: "Stocks", color: "#8cc1a5",
    items: ["Resource(0)", "Block(CopperOre)", "Block(Malachite)", "Block(Chalcopyrite)"],
    description: "Tall mineral bodies near magmatic centres.",
    values: [depth, ["Cell size (m)", /CELL: f32 = (\d+\.\d+)/, 48, 256],
      ["Base cell chance", /s\) > (\d+\.\d+) \+ magmatic/, 0, 0.5],
      ["Copper vein cutoff", /veinlets > (\d+\.\d+)/, 0, 1]] },
  { key: "tin_lode", label: "Tin", form: "Lodes", color: "#c4d7d8", items: ["Resource(1)", "Block(TinOre)", "Block(Cassiterite)"],
    description: "Steep mineral sheets along tectonic belts.",
    values: [depth, ["Lode spacing (m)", /SPACING: f32 = (\d+\.\d+)/, 4, 192],
      ["Tectonic cutoff", /boundary_proximity < (\d+\.\d+)/, 0, 2],
      ["Rich pocket cutoff", /shoot > (\d+\.\d+)/, 0, 1]] },
  { key: "geode", label: "Crystal", form: "Geodes", color: "#b2a3ed", items: ["Block(Crystal)", "Resource(5)"],
    description: "Hollow pockets lined with crystal in basalt or limestone.",
    values: [depth, ["Cell size (m)", /CELL: f32 = (\d+\.\d+)/, 16, 128],
      ["Geode cell chance", /s\) > (\d+\.\d+)/, 0, 1]] },
];

const decimal = value => Number.isInteger(value) ? value + ".0" :
  value.toLocaleString("en-US", { useGrouping: false, maximumSignificantDigits: 16 });

export function depositStudio(ctx, doc, spec, changed) {
  const deposit = deposits.find(item => item.key === spec.deposit);
  function region(source) {
    const start = source.indexOf("fn " + deposit.key + "(");
    const end = source.indexOf("\n}", start);
    if (start < 0 || end < 0) throw new Error("The deposit definition could not be found.");
    return { start, text: source.slice(start, end) };
  }
  let original = doc.text;
  const part = region(original);
  const controls = deposit.values.map(([title, pattern, min, max]) => {
    const match = part.text.match(pattern);
    if (!match) throw new Error("The generator changed. Open its source to edit " + title.toLowerCase() + ".");
    const input = el("input", { type: "number", value: match[1], min, max, step: "any", required: true });
    return { title, input, pattern };
  });
  const profile = el("canvas", { width: 640, height: 250, "aria-label": "Illustrated " + deposit.label + " deposit profile" });
  const readout = el("p", { className: "deposit-depth" });
  function draw() {
    const surface = 58;
    const start = Number(controls[0].input.value);
    const end = deposit.key === "coal_seam" ? Number(controls[1].input.value) : null;
    const range = Math.max(70, start + 30, end ?? 0);
    const y = surface + Math.min(1, start / range) * 155;
    const c = profile.getContext("2d");
    const width = profile.width;
    c.fillStyle = "#263c45"; c.fillRect(0, 0, width, surface);
    c.fillStyle = "#26303b"; c.fillRect(0, surface, width, 250 - surface);
    c.fillStyle = "#202731"; c.fillRect(0, 132, width, 118);
    c.fillStyle = "#37404a"; c.fillRect(0, 83, width, 31);
    c.beginPath(); c.moveTo(0, surface);
    for (let x = 0; x <= width; x += 16) c.lineTo(x, surface - Math.sin(x * 0.018) * 7 - Math.cos(x * .041) * 4);
    c.lineTo(width, surface + 12); c.lineTo(0, surface + 12); c.closePath();
    c.fillStyle = "#82936b"; c.fill();
    c.strokeStyle = "#abb8bd"; c.setLineDash([6, 6]); c.beginPath(); c.moveTo(0, y); c.lineTo(width, y); c.stroke(); c.setLineDash([]);
    c.fillStyle = deposit.color; c.strokeStyle = deposit.color;
    const floor = end ? surface + Math.min(1, end / range) * 155 : 232;
    if (["coal_seam", "iron_bands"].includes(deposit.key)) {
      for (let line = y + 9; line < floor; line += 25) {
        c.beginPath(); c.moveTo(35, line); c.bezierCurveTo(162, line - 15, 310, line + 18, 604, line - 5);
        c.lineWidth = deposit.key === "coal_seam" ? 3 : 5; c.stroke();
      }
    } else if (deposit.key === "tin_lode") {
      for (const x of [148, 321, 484]) {
        c.beginPath(); c.moveTo(x, y + 8); c.lineTo(x + 38, 233); c.lineWidth = 5; c.stroke();
      }
    } else {
      for (const [x, offset] of [[172, 18], [315, 42], [488, 25]]) {
        const geode = deposit.key === "geode";
        const centre = Math.min(214, y + offset + 11);
        c.beginPath(); c.ellipse(x, centre, geode ? 14 : 28, geode ? 12 : Math.min(32, centre - y), 0, 0, 2 * Math.PI);
        c.lineWidth = 5;
        if (geode) c.stroke(); else c.fill();
      }
    }
    c.fillStyle = "#f1e9dc"; c.font = "14px sans-serif";
    c.fillText("Surface", 17, 34);
    c.fillText(start + " m", 17, y - 7);
    readout.textContent = end ? start + "–" + end + " m below the surface" : "From " + start + " m below the surface";
  }
  const fields = el("div", { className: "deposit-fields" }, controls.map(({ title, input }) => el("label", {}, title, input)));
  fields.addEventListener("input", draw);
  draw();
  const apply = button("Review changes", () => {
    try {
      if (doc.text !== original) throw new Error("The generator draft changed. Reopen this deposit.");
      const values = controls.map(({ input }) => {
        if (!input.value || !input.validity.valid) throw new Error("Enter a value within range for each setting.");
        return Number(input.value);
      });
      if (deposit.key === "coal_seam" && values[0] >= values[1]) throw new Error("Coal must stop deeper than it starts.");
      const next = region(original);
      let text = next.text;
      controls.forEach(({ pattern }, i) => {
        const match = pattern.exec(text);
        if (Number(match[1]) === values[i]) return;
        const start = match.index + match[0].indexOf(match[1]);
        text = patch(text, start, start + match[1].length, decimal(values[i]));
      });
      if (text === next.text) {
        if (ctx.dirty(doc)) ctx.review(doc);
        else ctx.note("No deposit changes to review");
        return;
      }
      if (ctx.edit(doc, patch(original, next.start, next.start + next.text.length, text))) {
        original = doc.text;
        changed();
        ctx.review(doc);
      }
    } catch (error) { ctx.note(error.message, true); }
  }, "See the generator edits before saving", "primary");
  return el("div", { className: "deposit-studio" },
    el("section", { className: "world-card deposit-profile" }, el("div", { className: "world-card-head" },
      el("h3", {}, "Deposit profile"), el("span", { className: "muted" }, "Illustration")),
    profile, readout, el("p", { className: "muted" }, deposit.description)),
    el("section", { className: "world-card" }, el("h3", {}, "Distribution"), fields),
    el("div", { className: "world-actions" }, apply));
}
