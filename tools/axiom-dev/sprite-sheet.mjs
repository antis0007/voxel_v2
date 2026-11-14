import { patch } from "./syntax.mjs";

export const masksPath = "assets/tools/sprites16.txt";
const iconSheet = /^assets\/icons\/(?:items|machines|resources|effects)16\.txt$/;
const swatchKeys = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
export const isSheet = path => iconSheet.test(path) || path === masksPath;
export const isPng = path => /^assets\/.+\.png$/i.test(path);
export const isSprite = isSheet;
export function artSource(path) {
  if (isSheet(path)) return { mode: "source", width: 16, height: 16 };
  if (/\/comparison_[^/]+\.png$/i.test(path)) return { mode: "reference" };
  const generators = {
    "assets/viewmodel/kit.png": "tools/pixel_kit/atlas.py",
    "assets/source/textures/clean_modded_candidate/kit.png": "tools/pixel_kit/clean_atlas.py",
    "assets/runtime/viewmodels/wayfarer.png": "tools/blender/export_viewmodels.py",
    "crates/app/src/egui_ui/icons_spells.rs": "tools/gen_spell_icons.py",
  };
  const generator = generators[path];
  if (generator || /^assets\/(?:runtime|viewmodel)\/.*\.png$|\/(?:hand_)?atlas\.png$/.test(path)) {
    return { mode: "generated", generator };
  }
  if (isPng(path) && /^assets\/(?:source\/)?textures\//.test(path)) return { mode: "source" };
  return isPng(path) ? { mode: "reference" } : undefined;
}
export const hex = value => (value >>> 0).toString(16).padStart(8, "0");
export const rgba = (color, alpha = 255) => ((Number.parseInt(color, 16) << 8) | alpha) >>> 0;

export function paintInk(current, ink, indexed, lockAlpha) {
  if (!lockAlpha) return ink;
  if (indexed) return current === 46 || ink === 46 ? current : ink;
  if (!(current & 255) || !(ink & 255)) return current;
  return ((ink & 0xffffff00) | (current & 255)) >>> 0;
}

export function readSheet(text, path) {
  const masks = path === masksPath;
  const lines = [...text.matchAll(/[^\r\n]+/g)].map(match => ({
    text: match[0], start: match.index,
  })).filter(line => line.text.trim() && !line.text.startsWith(masks ? "//" : "#"));
  const entries = [];
  const ramps = [];
  const tools = [];
  let outline;
  let current;
  const color = /^[0-9a-f]{6}$/i;

  for (const line of lines) {
    const words = [...line.text.matchAll(/\S+/g)];
    const [head, a, b] = words.map(match => match[0]);
    const swatch = (key, slot) => ({
      key, rgb: words[slot][0].toLowerCase(), offset: line.start + words[slot].index, line: line.start,
    });
    const fail = () => { throw new Error("Invalid sprite sheet near " + line.text.slice(0, 80)); };
    if (!current) {
      if (masks && head === "palette" && words.length === 6 && words.slice(2).every(match => color.test(match[0]))) {
        ramps.push({ name: a, palette: [1, 2, 3, 4].map((key, slot) => swatch(String(key), slot + 2)) });
      } else if (masks && head === "tool" && a && words.slice(2).every(match => /^[^:]+:\d+$/.test(match[0]))) {
        tools.push({ name: a, layers: words.slice(2).map(match => match[0].split(":")[0]),
          slots: words.slice(2).map(match => Number(match[0].split(":")[1])) });
      } else if (!masks && head === "outline" && words.length === 2 && color.test(a)) {
        outline = { ...swatch("o", 1), shared: true };
      } else if (masks ? head === "mask" && words.length === 2
        : head === "sprite" && words.length === 3 && /^\d+$/.test(a) && outline) {
        current = { id: masks ? a : a + " · " + b, name: masks ? a : b,
          palette: masks ? [] : [outline], rows: [] };
      } else fail();
      continue;
    }
    if (head === "end" && words.length === 1) {
      if (current.rows.length !== 16) fail();
      current.end = line.start + line.text.length;
      entries.push(current);
      current = undefined;
      continue;
    }
    if (!masks && words.length === 2 && /^[!-~]$/.test(head) && color.test(a)) {
      if (current.palette.some(item => item.key === head) || head === ".") fail();
      current.palette.push(swatch(head, 1));
      continue;
    }
    const allowed = masks ? ".1234" : "." + current.palette.map(item => item.key).join("");
    if (line.text.length !== 16 || [...line.text].some(key => !allowed.includes(key))) fail();
    current.rows.push(line);
  }
  if (current || !entries.length || (masks && !ramps.length)) throw new Error("Incomplete sprite sheet");
  return { masks, entries, ramps, tools };
}

export function cells(entry) {
  return Uint32Array.from(entry.rows.flatMap(row => [...row.text].map(key => key.charCodeAt(0))));
}

export function composeTool(sheet, tool, selected, draft, resolve) {
  const result = new Uint32Array(256);
  for (const mask of tool.layers) {
    const entry = sheet.entries.find(candidate => candidate.name === mask);
    if (!entry) continue;
    const source = entry === selected ? draft : cells(entry);
    source.forEach((value, index) => {
      if (value !== 46) result[index] = resolve(value);
    });
  }
  const silhouette = result.slice();
  silhouette.forEach((value, i) => {
    const x = i % 16;
    if (!value && ((x > 0 && silhouette[i - 1]) || (x < 15 && silhouette[i + 1])
      || silhouette[i - 16] || silhouette[i + 16])) result[i] = rgba("1a120e");
  });
  return result;
}

export function updateRows(text, entry, pixels) {
  for (let y = 15; y >= 0; y--) {
    const { start } = entry.rows[y];
    const row = String.fromCharCode(...pixels.subarray(y * 16, (y + 1) * 16));
    text = patch(text, start, start + 16, row);
  }
  return text;
}

export function addSwatch(text, entry, rgb) {
  const key = [...swatchKeys]
    .find(candidate => !entry.palette.some(item => item.key === candidate));
  if (!key) throw new Error("The sprite palette is full");
  const start = entry.rows[0].start;
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  return { key, text: patch(text, start, start, key + " " + rgb + newline) };
}

export function applySelectedColors(text, entry, pixels, adjusted, resolve) {
  const byColor = new Map(entry.palette.map(swatch => [swatch.rgb, swatch.key]));
  const used = new Set(entry.palette.map(swatch => swatch.key));
  const additions = [];
  const next = pixels.map((pixel, index) => {
    if (adjusted[index] === resolve(pixel)) return pixel;
    const color = hex(adjusted[index]).slice(0, 6);
    let key = byColor.get(color);
    if (!key) {
      key = [...swatchKeys].find(candidate => !used.has(candidate));
      if (!key) throw new Error("The selected colors exceed this sprite’s palette capacity.");
      byColor.set(color, key);
      used.add(key);
      additions.push(key + " " + color);
    }
    return key.charCodeAt(0);
  });
  const edited = updateRows(text, entry, next);
  if (!additions.length) return edited;
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  const at = entry.rows[0].start;
  return patch(edited, at, at, additions.join(newline) + newline);
}

export function shiftHue(pixel, degrees) {
  const rgb = [pixel >>> 24, (pixel >>> 16) & 255, (pixel >>> 8) & 255];
  const low = Math.min(...rgb), high = Math.max(...rgb), span = high - low;
  if (!span || !degrees || !(pixel & 255)) return pixel;
  const [r, g, b] = rgb;
  const hue = high === r ? (g - b) / span : high === g ? 2 + (b - r) / span : 4 + (r - g) / span;
  const shifted = ((hue + degrees / 60) % 6 + 6) % 6;
  const channel = offset => Math.round(low + span * Math.max(0, Math.min(1, Math.abs((shifted + offset) % 6 - 3) - 1)));
  return ((channel(0) << 24) | (channel(4) << 16) | (channel(2) << 8) | (pixel & 255)) >>> 0;
}

export function tintedPalette(entry, degrees, outline = entry.palette[0]) {
  return [outline, ...entry.palette.filter(swatch => !swatch.shared).map(swatch => ({
    key: swatch.key, rgb: hex(shiftHue(rgba(swatch.rgb), degrees)).slice(0, 6),
  }))];
}

export function tintedSprite(text, target, source, degrees) {
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  const palette = tintedPalette(source, degrees).slice(1).map(({ key, rgb }) => key + " " + rgb);
  const body = [...palette, ...source.rows.map(row => row.text), "end"].join(newline);
  if (typeof target === "string") {
    return text.trimEnd() + newline + newline + "sprite " + target + newline + body + newline;
  }
  return replaceIconBody(text, target, body);
}

function replaceIconBody(text, entry, body) {
  return patch(text, entry.palette[1]?.line ?? entry.rows[0].start, entry.end, body);
}

// Replace one icon with an original 16 × 16 RGBA design, keeping its stable ID.
export function replaceIconPixels(text, entry, pixels) {
  if (pixels.length !== 256 || !entry.palette[0]?.shared) throw new Error("Choose a native icon and 16 × 16 art");
  const outline = entry.palette[0];
  const palette = new Map([[outline.rgb, outline.key]]);
  const keys = [...swatchKeys].filter(key => key !== outline.key);
  const rows = Array.from({ length: 16 }, (_, y) => Array.from(pixels.subarray(y * 16, y * 16 + 16), color => {
    if (!(color & 255)) return ".";
    const rgb = hex(color).slice(0, 6);
    if (!palette.has(rgb)) {
      const key = keys.shift();
      if (!key) throw new Error("The design exceeds the icon palette");
      palette.set(rgb, key);
    }
    return palette.get(rgb);
  }).join(""));
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const swatches = [...palette].slice(1).map(([rgb, key]) => key + " " + rgb);
  const body = [...swatches, ...rows, "end"].join(eol);
  return replaceIconBody(text, entry, body);
}

