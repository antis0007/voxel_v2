import { el } from "./inspector.mjs";
import { canvasPixels } from "./pixel-grid.mjs";
import { cells, rgba, tintedPalette } from "./sprite-sheet.mjs";

export const label = (name, input) => el("label", {}, name, input);

export function hueControl(value, redraw) {
  const input = el("input", { type: "range", min: -180, max: 180, step: 1, value: String(value) });
  const reading = el("output", {}, value + "°");
  input.oninput = () => { reading.textContent = input.value + "°"; redraw(); };
  return { input, view: el("div", { className: "tint-hue" }, label("Hue shift", input), reading) };
}

export function tintPreviews(after = "Tinted copy", before = "Source") {
  const canvases = [before, after].map(name => el("canvas", { "aria-label": name + " preview" }));
  const view = el("div", { className: "tint-previews" }, canvases.map((canvas, i) =>
    el("figure", {}, canvas, el("figcaption", {}, i ? after : before))));
  return {
    view,
    sprite(entry, degrees, outline) {
      const pixels = cells(entry);
      [entry.palette, tintedPalette(entry, degrees, outline)].forEach((palette, i) => {
        const colors = new Map(palette.map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
        canvasPixels(canvases[i], pixels, 16, 16, pixel => colors.get(pixel) ?? 0);
      });
    },
    pixels(before, after, width, height) {
      canvasPixels(canvases[0], before, width, height);
      canvasPixels(canvases[1], after, width, height);
    },
  };
}
