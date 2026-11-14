import { el, button } from "./inspector.mjs";
import { canvasPixels, line, pngDimensions } from "./pixel-grid.mjs";
import { readSheet, replaceIconPixels } from "./sprite-sheet.mjs";

const SIZE = 16;
const sheets = ["machines", "items", "resources", "effects"];
const label = (text, control) => el("label", {}, el("span", {}, text), control);
const option = (value, text) => el("option", { value }, text);

function referencePalette(data) {
  const histogram = new Map();
  let opaque = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 128) continue;
    opaque++;
    const channels = [data[i], data[i + 1], data[i + 2]];
    const key = channels.reduce((id, channel) => id * 16 + (channel >> 4), 0);
    const bin = histogram.get(key) ?? [0, 0, 0, 0];
    bin[0]++;
    channels.forEach((channel, c) => { bin[c + 1] += channel; });
    histogram.set(key, bin);
  }
  const colors = [...histogram.values()].sort((a, b) => b[0] - a[0]).slice(0, 6).map(([count, ...rgb]) =>
    rgb.map(channel => Math.round(channel / count)));
  return { colors, coverage: Math.round(opaque * 400 / data.length) };
}

function generatedPixels(colors, motif, seed) {
  const pixels = new Uint32Array(SIZE * SIZE);
  const base = colors[0] ?? [91, 115, 124];
  const accent = colors.slice(1).sort((a, b) => chroma(b) - chroma(a))[0] ?? [77, 181, 191];
  const pack = rgb => (((rgb[0] << 24) | (rgb[1] << 16) | (rgb[2] << 8) | 255) >>> 0);
  const shade = (rgb, mul, add = 0) => pack(rgb.map(channel => Math.max(0, Math.min(255, Math.round(channel * mul + add)))));
  const shades = [shade(base, 0.62), shade(base, 0.85), shade(base, 1.03), shade(base, 1.14)];
  const light = shade(accent, 0.92, 34);
  const shadow = shade(accent, 0.53);
  const outline = shade(base, 0.31, 7);
  const mark = (x, y, color) => { pixels[y * SIZE + x] = color; };
  let hash = 2166136261;
  for (const char of seed) hash = Math.imul(hash ^ char.codePointAt(0), 16777619);
  const noise = (x, y) => {
    const value = Math.imul(x + 1, 374761393) ^ Math.imul(y + 1, 668265263) ^ hash;
    return (Math.imul(value ^ (value >>> 13), 1274126177) >>> 24);
  };
  if (motif === "rune") {
    const symbols = [
      [[8, 1], [13, 7], [8, 14], [2, 8], [8, 1]],
      [[8, 4], [11, 8], [8, 11], [5, 8], [8, 4]],
      [[2, 8], [5, 8]], [[11, 8], [13, 7]],
    ];
    for (const points of symbols) {
      for (let i = 1; i < points.length; i++) {
        const a = points[i - 1], b = points[i];
        line(...a, ...b, (x, y) => { mark(x, y + 1, shadow); mark(x, y, light); });
      }
    }
    return pixels;
  }
  for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
      const random = noise(x, y);
      if (motif === "ore") {
        const vein = Math.abs(((x * 3 + y * 5 + (hash & 15)) % 16) - 8);
        mark(x, y, vein < 2 && random > 87 ? (random > 172 ? light : shadow) : shades[random >> 6]);
      } else {
        const border = x === 0 || y === 0 || x === 15 || y === 15;
        const rivet = (x === 2 || x === 13) && (y === 2 || y === 13);
        const vent = x >= 4 && x <= 11 && (y === 5 || y === 8 || y === 11);
        mark(x, y, border ? outline : rivet ? light : vent ? shadow : shades[y === 1 || x === 1 ? 3 : 1 + (random >> 7)]);
      }
    }
  }
  return pixels;
}

const chroma = channels => Math.max(...channels) - Math.min(...channels);
const rgbHex = rgb => rgb.map(channel => channel.toString(16).padStart(2, "0")).join("");

export function openTextureStudy(ctx, mod, onEvidence) {
  const source = el("input", { type: "text", list: "texture-study-paths", placeholder: "assets/modid/textures/…", maxlength: 400 });
  const upload = el("input", { type: "file", accept: "image/png,.png" });
  const motif = el("select", {}, option("panel", "Machine panel"), option("ore", "Mineral vein"), option("rune", "Rune icon"));
  const seed = el("input", { type: "text", value: "Axiom", maxlength: 80 });
  const tint = el("input", { type: "color", value: "#5b737c" });
  const sheet = el("select", {}, option("", "PNG export only"), ...sheets.map(name => option(name, name)));
  const icon = el("select");
  const original = el("canvas", { "aria-label": "Local texture reference" });
  const candidate = el("canvas", { "aria-label": "Original Axiom texture draft" });
  const evidence = el("p", { className: "muted" }, "Choose a reference or set a base color.");
  let palette = [], pixels, doc;
  function draw() {
    const base = [1, 3, 5].map(start => Number.parseInt(tint.value.slice(start, start + 2), 16));
    pixels = generatedPixels([base, ...palette.slice(1)], motif.value, seed.value);
    canvasPixels(candidate, pixels, SIZE, SIZE);
  }
  async function loadReference() {
    let blob = upload.files[0], name = blob?.name;
    if (source.value.trim() && mod) {
      name = source.value.trim();
      const result = await ctx.api("/api/mod-texture", name, { digest: mod.digest });
      blob = new Blob([Uint8Array.from(atob(result.png), value => value.charCodeAt(0))], { type: "image/png" });
    }
    if (!blob || blob.size > 256 * 1024) throw new Error("Choose a PNG reference under 256 KiB");
    const { width, height } = pngDimensions(new DataView(await blob.arrayBuffer()));
    if (width > 512 || height > 512) throw new Error("Choose a PNG up to 512 × 512");
    const image = await createImageBitmap(blob);
    try {
      original.width = width;
      original.height = height;
      const canvas = original.getContext("2d");
      canvas.drawImage(image, 0, 0);
      const { colors, coverage } = referencePalette(canvas.getImageData(0, 0, width, height).data);
      palette = colors;
      if (colors.length) tint.value = "#" + rgbHex(colors[0]);
      seed.value = name.split("/").at(-1).slice(0, 80);
      const hues = colors.map(rgb => "#" + rgbHex(rgb)).join(", ");
      evidence.textContent = `${name} · ${width} × ${height} · ${coverage}% opaque · Palette ${hues || "empty"}`;
      onEvidence?.(evidence.textContent);
      draw();
    } finally {
      image.close();
    }
  }
  const load = button("Read Reference", async () => {
    load.disabled = true;
    try { await loadReference(); } catch (error) { ctx.note(error.message, true); }
    finally { load.disabled = false; }
  });
  sheet.onchange = async () => {
    const selected = sheet.value;
    doc = undefined;
    icon.replaceChildren();
    if (!selected) return;
    try {
      const next = await ctx.load("assets/icons/" + selected + "16.txt");
      if (sheet.value !== selected) return;
      const entries = readSheet(next.text, next.path).entries;
      icon.replaceChildren(option("", "Choose an icon"), ...entries.map(entry => option(entry.id, entry.name)));
      doc = next;
    } catch (error) { ctx.note(error.message, true); }
  };
  motif.onchange = seed.oninput = tint.oninput = draw;
  const download = button("Download PNG", () => candidate.toBlob(blob => {
    if (!blob) return;
    const url = URL.createObjectURL(blob);
    el("a", { href: url, download: "axiom-" + motif.value + ".png" }).click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }));
  original.width = original.height = SIZE;
  draw();
  ctx.dialog("Texture study", [
    el("div", { className: "idea-fields" },
      mod ? label("JAR texture path", source) : null,
      mod ? el("datalist", { id: "texture-study-paths" }, mod.paths.map(path => option(path, path))) : null,
      label("Or local PNG", upload), load, label("Motif", motif), label("Base color", tint), label("Variant seed", seed),
      label("Axiom sprite sheet", sheet), label("Target icon", icon)),
    el("div", { className: "tint-previews" }, original, candidate), evidence, download,
    el("p", { className: "muted" }, "The draft uses a generated pattern and palette cues. Source pixels stay in the reference preview."),
  ], "Stage sprite draft", () => {
    const entry = doc && readSheet(doc.text, doc.path).entries.find(entry => entry.id === icon.value);
    if (!entry) throw new Error("Choose a native icon, or download the PNG");
    if (!ctx.edit(doc, replaceIconPixels(doc.text, entry, pixels))) return false;
    ctx.note("Sprite draft staged · " + entry.name);
    return true;
  });
}
