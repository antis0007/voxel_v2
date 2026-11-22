import { openNormalLab } from "./normal-lab.mjs";
import { el, button, panel } from "./inspector.mjs";
import { patch } from "./syntax.mjs";
import { isSheet, readSheet, cells, updateRows, addSwatch, applySelectedColors, composeTool, hex, rgba, shiftHue, tintedSprite, paintInk } from "./sprite-sheet.mjs";
import { loadPng, canvasPixels, readTile, writeTile, pngChanges, fill, line } from "./pixel-grid.mjs";
import { adjustColor, paletteCandidates, varyColor } from "./sprite-color.mjs";
import { hueControl, tintPreviews } from "./tint-controls.mjs";

const tools = [["Pencil", "B"], ["Erase", "E"], ["Fill", "G"], ["Pick", "I"], ["Line", "L"],
  ["Rectangle", "R"], ["Ellipse", "O"], ["Dither", "D"], ["Replace", "X"], ["Marquee", "M"]];
const shapeTools = new Set(["Line", "Rectangle", "Ellipse"]);
const label = (name, control) => el("label", { className: "sprite-label" }, name, control);
const number = (value, min, max) => el("input", { type: "number", value: String(value), min, max, step: 1 });
const choice = (value, text) => el("option", { value }, text);
const getNumber = (control, fallback) => control.validity.valid && control.value !== "" ? +control.value : fallback;

function browseThumbnails(title, count, selected, describe, draw, onSelect, searchable = true) {
  const perPage = 48;
  const search = el("input", { type: "search", placeholder: "Search by name or ID…", "aria-label": "Search sprites" });
  const grid = el("div", { className: "sprite-picker-grid" });
  const status = el("span", { className: "muted", role: "status" });
  let matches;
  let page = Math.floor(selected / perPage);
  const previous = button("Previous", () => { page--; render(); });
  const next = button("Next", () => { page++; render(); });
  const dialog = el("dialog", { className: "sprite-picker", "aria-label": title, onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, el("h2", {}, title), button("Close", () => dialog.close())),
    searchable ? search : null, grid, el("footer", { className: "sprite-picker-pages" }, previous, status, next));

  function render() {
    const total = matches?.length ?? count;
    const start = page * perPage;
    const end = Math.min(start + perPage, total);
    grid.replaceChildren(...Array.from({ length: end - start }, (_, i) => {
      const index = matches ? matches[start + i] : start + i;
      const name = describe(index);
      const thumbnail = el("canvas", { className: "sprite-thumb", "aria-hidden": "true" });
      draw(thumbnail, index);
      const tile = button("", () => { dialog.close(); onSelect(index); }, name, "sprite-picker-tile");
      tile.append(thumbnail, el("span", {}, name));
      if (index === selected) tile.setAttribute("aria-current", "true");
      return tile;
    }));
    previous.disabled = !page;
    next.disabled = end >= total;
    status.textContent = total ? (start + 1) + "–" + end + " of " + total : "No matches";
    grid.scrollTop = 0;
  }
  search.oninput = () => {
    const terms = search.value.toLowerCase().split(/\s+/).filter(Boolean);
    matches = terms.length ? Array.from({ length: count }, (_, i) => i)
      .filter(i => terms.every(term => describe(i).toLowerCase().includes(term))) : undefined;
    page = 0;
    render();
  };
  search.onkeydown = event => { if (event.key === "Enter") grid.firstElementChild?.click(); };
  render();
  document.body.append(dialog);
  dialog.showModal();
  if (searchable) search.focus();
}

export function spriteView(doc, ui) {
  const host = el("div", { className: "sprite-editor" }, el("p", { className: "muted" }, "Opening pixels…"));
  async function open() {
    try {
      const sheet = isSheet(doc.path) ? readSheet(doc.text, doc.path) : null;
      if (!sheet && !doc.spriteBitmap) {
        if (!doc.version) throw new Error("This file exceeds the image size limit");
        const blob = await ui.api("/api/preview", doc.path, undefined, true);
        const bitmap = await loadPng(blob);
        if (!host.isConnected) return;
        doc.spriteBitmap ??= bitmap;
      }
      host.replaceChildren(build(doc, ui, sheet));
    } catch (error) {
      host.replaceChildren(panel("Sprite editor", el("p", { className: "error" }, error.message)));
    }
  }
  queueMicrotask(open);
  return host;
}

function build(doc, ui, sheet) {
  const settings = doc.spriteSettings = {
    tool: "Pencil", brush: 1, mirrorX: false, mirrorY: false, solid: false, lockAlpha: false, key: "", color: "df905f", alpha: 255, grid: true, zoom: 0, entry: 0, ramp: 0, actualSize: true,
    tileWidth: doc.spriteBitmap?.width, tileHeight: doc.spriteBitmap?.height, column: 0, row: 0,
    ...doc.spriteSettings,
  };
  const bitmap = doc.spriteBitmap;
  const selector = el("div", { className: "sprite-selection" });
  const info = el("p", { className: "muted" });
  const palette = el("div", { className: "sprite-palette" });
  const bar = el("div", { className: "sprite-toolbar" });
  const canvas = el("canvas", { tabIndex: 0, "aria-label": "Sprite painting canvas" });
  const native = document.createElement("canvas");
  const preview = el("canvas", { className: "sprite-preview", "aria-label": "Sprite preview" });
  function download(blob, name) {
    if (!blob) return ui.note("Could not export this sprite.", true);
    const url = URL.createObjectURL(blob);
    const link = el("a", { href: url, download: name });
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  const assembled = sheet?.masks ? el("canvas", { className: "sprite-preview",
    "aria-label": "Assembled tool preview", title: "All parts use the selected material ramp" }) : null;
  const assembly = assembled ? el("select", { onchange: event => {
    settings.assembly = event.target.value;
    repaint();
  } }) : null;
  const position = el("span", { className: "muted" });
  const stage = el("div", { className: "sprite-stage" }, canvas);
  let rect, pixels, resolve, entry, swatches, zoom, stroke;
  let frame, rotateButton, selection, clipboard, pasteArmed;
  const context = canvas.getContext("2d");

  const toolButtons = tools.map(([tool, key]) => button(tool, () => setTool(tool), tool + " (" + key + ")"));
  function setTool(tool) {
    settings.tool = tool;
    for (let i = 0; i < tools.length; i++) {
      const active = tools[i][0] === tool;
      toolButtons[i].classList.toggle("active", active);
      toolButtons[i].setAttribute("aria-pressed", String(active));
    }
  }

  function repaint() {
    if (frame !== undefined) cancelAnimationFrame(frame);
    frame = undefined;
    canvasPixels(native, pixels, rect.width, rect.height, resolve);
    canvas.width = rect.width * zoom;
    canvas.height = rect.height * zoom;
    context.imageSmoothingEnabled = false;
    context.drawImage(native, 0, 0, canvas.width, canvas.height);
    if (settings.grid && zoom >= 5) {
      context.strokeStyle = "#24324366";
      context.beginPath();
      for (let x = zoom; x < canvas.width; x += zoom) { context.moveTo(x + .5, 0); context.lineTo(x + .5, canvas.height); }
      for (let y = zoom; y < canvas.height; y += zoom) { context.moveTo(0, y + .5); context.lineTo(canvas.width, y + .5); }
      context.stroke();
    }
    if (selection) {
      const { x, y, width, height } = selection;
      context.strokeStyle = "#111";
      context.strokeRect(x * zoom + .5, y * zoom + .5, Math.max(1, width * zoom - 1), Math.max(1, height * zoom - 1));
      context.strokeStyle = "#fff";
      context.setLineDash([3, 3]);
      context.strokeRect(x * zoom + .5, y * zoom + .5, Math.max(1, width * zoom - 1), Math.max(1, height * zoom - 1));
      context.setLineDash([]);
    }
    const scale = Math.min(1, 128 / Math.max(rect.width, rect.height));
    preview.width = Math.max(1, Math.round(rect.width * scale));
    preview.height = Math.max(1, Math.round(rect.height * scale));
    preview.classList.toggle("actual-size", settings.actualSize);
    const sample = preview.getContext("2d");
    sample.imageSmoothingEnabled = false;
    sample.drawImage(native, 0, 0, preview.width, preview.height);
    if (assembly) {
      const tool = sheet.tools.find(candidate => candidate.name === assembly.value);
      assembled.hidden = !tool;
      if (tool) canvasPixels(assembled, composeTool(sheet, tool, entry, pixels, resolve), 16, 16);
    }
  }

  function queuePaint() {
    frame ??= requestAnimationFrame(repaint);
  }

  function viewPixels() {
    selection = undefined;
    pasteArmed = false;
    if (sheet) {
      if (entry !== sheet.entries[settings.entry]) clipboard = undefined;
      entry = sheet.entries[settings.entry] ?? sheet.entries[0];
      swatches = sheet.masks ? sheet.ramps[settings.ramp]?.palette ?? sheet.ramps[0].palette : entry.palette;
      const mapped = new Map(swatches.map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
      resolve = value => mapped.get(value) ?? 0;
      rect = { x: 0, y: 0, width: 16, height: 16 };
      pixels = cells(entry);
      if (!swatches.some(item => item.key === settings.key) && settings.key !== ".") {
        settings.key = swatches.find(item => !item.shared)?.key ?? swatches[0]?.key ?? ".";
      }
    } else {
      rect = { x: settings.column * settings.tileWidth, y: settings.row * settings.tileHeight,
        width: settings.tileWidth, height: settings.tileHeight };
      rect.width = Math.min(rect.width, bitmap.width - rect.x);
      rect.height = Math.min(rect.height, bitmap.height - rect.y);
      pixels = readTile(bitmap, rect);
      resolve = undefined;
    }
    selection = doc.spriteSelection?.id === selectionId() ? doc.spriteSelection.bounds : undefined;
    if (rotateButton) {
      rotateButton.disabled = rect.width !== rect.height;
      updateSelectionActions();
    }
    updateZoom();
  }
  const selectionId = () => [entry?.id, rect.x, rect.y, rect.width, rect.height].join("/");

  function updateZoom() {
    const maximum = Math.max(1, Math.floor(3072 / Math.max(rect.width, rect.height)));
    const width = stage.clientWidth ? stage.clientWidth - 28 : 420;
    const height = stage.clientHeight ? stage.clientHeight - 28 : Math.max(144, window.innerHeight - 294);
    const fit = Math.max(1, Math.min(32, Math.floor(width / rect.width), Math.floor(height / rect.height)));
    zoom = Math.min(maximum, settings.zoom || fit);
    info.textContent = rect.width + " × " + rect.height + " px · " + zoom + "× zoom"
      + (bitmap ? " · origin " + rect.x + ", " + rect.y : "");
    repaint();
  }

  const zoomControl = el("select", { onchange: event => {
    settings.zoom = +event.target.value;
    updateZoom();
  } }, choice("0", "Auto"), [1, 2, 4, 8, 12, 16, 24, 32].map(value => choice(String(value), value + "×")));
  zoomControl.value = String(settings.zoom);
  const grid = el("input", { type: "checkbox", checked: settings.grid, onchange: event => {
    settings.grid = event.target.checked;
    repaint();
  } });

  function paletteControls() {
    if (sheet) {
      const selectInk = key => {
        settings.key = key;
        if (settings.tool === "Pick" || settings.tool === "Erase") setTool("Pencil");
        paletteControls();
      };
      const clear = button("∅", () => selectInk("."), "Transparent", "sprite-swatch transparent");
      clear.setAttribute("aria-label", "Transparent");
      clear.classList.toggle("active", settings.key === ".");
      clear.setAttribute("aria-pressed", String(settings.key === "."));
      const colors = swatches.map(({ key, rgb, shared }) => {
        const scope = shared ? "Shared outline for this sheet" : sheet.masks
          ? "Shared material ramp, shade " + key : "Sprite palette key " + key;
        const swatch = button(key, () => selectInk(key), scope + " · #" + rgb,
          "sprite-swatch" + (settings.key === key ? " active" : ""));
        swatch.setAttribute("aria-label", scope);
        swatch.setAttribute("aria-pressed", String(settings.key === key));
        swatch.style.backgroundColor = "#" + rgb;
        return swatch;
      });
      const selected = swatches.find(item => item.key === settings.key);
      palette.replaceChildren(el("div", { className: "swatches" }, clear, colors));
      if (selected) {
        const scope = selected.shared ? "Edit shared outline" : sheet.masks ? "Edit shared shade" : "Edit color";
        const picker = el("input", { type: "color", value: "#" + selected.rgb, title: scope,
          onchange: event => ui.update(patch(doc.text, selected.offset, selected.offset + 6, event.target.value.slice(1))) });
        palette.append(el("div", { className: "swatch-edit" }, label(scope, picker),
          el("span", { className: "muted" }, "#" + selected.rgb.toUpperCase())));
      }
      if (!sheet.masks) {
        palette.append(button("Add color", () => {
          try {
            const added = addSwatch(doc.text, entry, selected?.rgb ?? settings.color);
            settings.key = added.key;
            ui.update(added.text);
          } catch (error) { ui.note(error.message, true); }
        }, "Add an independent color to this sprite"));
      }
    } else {
      const color = el("input", { type: "color", value: "#" + settings.color, oninput: event => {
        settings.color = event.target.value.slice(1); if (settings.tool === "Pick" || settings.tool === "Erase") setTool("Pencil");
      } });
      const alpha = number(settings.alpha, 0, 255);
      alpha.onchange = () => { settings.alpha = getNumber(alpha, settings.alpha); alpha.value = settings.alpha; };
      const counts = new Map();
      const stride = Math.max(1, Math.ceil(pixels.length / 4096));
      for (let i = 0; i < pixels.length; i += stride) {
        const pixel = pixels[i];
        if (pixel & 255) counts.set(pixel, (counts.get(pixel) ?? 0) + 1);
      }
      const sampled = [...counts].sort((a, b) => b[1] - a[1]).slice(0, 18).map(([pixel]) => {
        const rgb = hex(pixel).slice(0, 6);
        const swatchAlpha = pixel & 255;
        const name = "#" + rgb.toUpperCase() + " · alpha " + swatchAlpha;
        const swatch = button("", () => {
          settings.color = rgb;
          settings.alpha = swatchAlpha;
          if (settings.tool === "Pick" || settings.tool === "Erase") setTool("Pencil");
          paletteControls();
        }, name, "sprite-swatch");
        swatch.setAttribute("aria-label", name);
        swatch.style.backgroundColor = "#" + hex(pixel);
        return swatch;
      });
      palette.replaceChildren(label("Color", color), label("Alpha (0–255)", alpha),
        sampled.length ? el("div", { className: "swatches", "aria-label": "Colors sampled from the tile" }, sampled) : null);
    }
  }

  if (sheet) {
    settings.entry = Math.min(settings.entry, sheet.entries.length - 1);
    settings.ramp = Math.min(settings.ramp, Math.max(0, sheet.ramps.length - 1));
    const spriteName = index => sheet.entries[index].name.replace(/_/g, " ");
    const findLabel = sheet.masks ? "Find part mask" : "Find sprite";
    const picker = button(spriteName(settings.entry) + "  ▾", () => {
      browseThumbnails(findLabel, sheet.entries.length, settings.entry,
        index => spriteName(index) + (sheet.masks ? "" : " · #" + sheet.entries[index].id.split(" · ")[0]),
        (thumbnail, index) => {
          const item = sheet.entries[index];
          const colors = new Map((sheet.masks ? swatches : item.palette)
            .map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
          canvasPixels(thumbnail, cells(item), 16, 16, pixel => colors.get(pixel) ?? 0);
        }, key => {
          settings.entry = key;
          picker.textContent = spriteName(key) + "  ▾";
          picker.setAttribute("aria-label", findLabel + " · " + spriteName(key));
          viewPixels();
          paletteControls();
          showEntry();
          ui.remember(doc);
        });
    }, findLabel + " in this sheet", "quick-find");
    picker.setAttribute("aria-label", findLabel + " · " + spriteName(settings.entry));
    picker.setAttribute("aria-haspopup", "dialog");
    function showEntry() {
      if (!assembly) return;
      const options = sheet.tools.filter(tool => tool.layers.includes(entry.name));
      assembly.replaceChildren(...(options.length ? options.map(tool => choice(tool.name, tool.name))
        : [choice("", "Unused mask")]));
      assembly.disabled = !options.length;
      repaint();
    }
    selector.append(label(sheet.masks ? "Part mask" : "Sprite", picker));
    if (sheet.masks) {
      const ramp = el("select", { onchange: event => {
        settings.ramp = +event.target.value;
        viewPixels();
        paletteControls();
        ui.remember(doc);
      } }, sheet.ramps.map((item, i) => choice(String(i), item.name)));
      ramp.value = String(settings.ramp);
      selector.append(label("Material ramp", ramp), label("Preview tool", assembly));
    }
    viewPixels();
    showEntry();
  } else {
    settings.tileWidth = Math.max(1, Math.min(bitmap.width, settings.tileWidth));
    settings.tileHeight = Math.max(1, Math.min(bitmap.height, settings.tileHeight));
    const width = number(settings.tileWidth, 1, bitmap.width);
    const height = number(settings.tileHeight, 1, bitmap.height);
    const column = number(settings.column + 1, 1, 1);
    const row = number(settings.row + 1, 1, 1);
    function atlas() {
      settings.tileWidth = getNumber(width, settings.tileWidth);
      settings.tileHeight = getNumber(height, settings.tileHeight);
      width.value = settings.tileWidth;
      height.value = settings.tileHeight;
      column.max = Math.ceil(bitmap.width / settings.tileWidth);
      row.max = Math.ceil(bitmap.height / settings.tileHeight);
      settings.column = Math.min(+column.max, Math.max(1, Math.floor(+column.value || 1))) - 1;
      settings.row = Math.min(+row.max, Math.max(1, Math.floor(+row.value || 1))) - 1;
      column.value = settings.column + 1;
      row.value = settings.row + 1;
      viewPixels();
    }
    const selectTile = () => { atlas(); paletteControls(); ui.remember(doc); };
    for (const control of [width, height, column, row]) control.onchange = selectTile;
    const browseTiles = button("Browse tiles", () => {
      const columns = +column.max;
      const atlasCanvas = document.createElement("canvas");
      canvasPixels(atlasCanvas, bitmap.pixels, bitmap.width, bitmap.height);
      browseThumbnails("Atlas tiles", columns * +row.max, settings.row * columns + settings.column,
        index => "Tile " + (index % columns + 1) + ", " + (Math.floor(index / columns) + 1),
        (thumbnail, index) => {
          const x = (index % columns) * settings.tileWidth;
          const y = Math.floor(index / columns) * settings.tileHeight;
          const w = Math.min(settings.tileWidth, bitmap.width - x);
          const h = Math.min(settings.tileHeight, bitmap.height - y);
          const scale = Math.min(1, 64 / Math.max(w, h));
          thumbnail.width = Math.max(1, Math.round(w * scale));
          thumbnail.height = Math.max(1, Math.round(h * scale));
          const context = thumbnail.getContext("2d");
          context.imageSmoothingEnabled = false;
          context.drawImage(atlasCanvas, x, y, w, h, 0, 0, thumbnail.width, thumbnail.height);
        }, index => {
          column.value = index % columns + 1;
          row.value = Math.floor(index / columns) + 1;
          selectTile();
        }, false);
    });
    selector.append(browseTiles, el("div", { className: "tile-controls" }, label("Tile width", width), label("Tile height", height),
      label("Column", column), label("Row", row)),
      button("Whole image", () => {
        width.value = bitmap.width; height.value = bitmap.height; column.value = row.value = 1; selectTile();
      }, "Edit the whole PNG without changing its dimensions"));
    atlas();
  }

  function commit(before) {
    if (!pixels.some((value, i) => value !== before[i])) return;
    if (sheet) {
      const next = updateRows(doc.text, entry, pixels);
      if (ui.update(next, true)) {
        // Row edits preserve offsets, so the picker and assembled preview can reuse this sheet.
        for (const row of entry.rows) row.text = next.slice(row.start, row.start + 16);
      }
    } else {
      bitmap.undo.push({ rect, pixels: before });
      let bytes = 0;
      let first = bitmap.undo.length;
      while (first && bytes < 8_388_608 && bitmap.undo.length - first < 40) bytes += bitmap.undo[--first].pixels.byteLength;
      bitmap.undo.splice(0, first);
      bitmap.redo.length = 0;
      writeTile(bitmap, rect, pixels);
      bitmap.changes = pngChanges(bitmap);
      paletteControls();
      ui.changed();
    }
  }

  function point(event) {
    const bounds = canvas.getBoundingClientRect();
    const x = Math.floor((event.clientX - bounds.left) * rect.width / bounds.width);
    const y = Math.floor((event.clientY - bounds.top) * rect.height / bounds.height);
    return [Math.max(0, Math.min(rect.width - 1, x)), Math.max(0, Math.min(rect.height - 1, y))];
  }

  function plot(x, y, ink) {
    const { width, height } = rect;
    if (x < 0 || x >= width || y < 0 || y >= height) return;
    const set = (px, py) => {
      if (contains(px, py)) {
        const i = py * width + px;
        pixels[i] = paintInk(pixels[i], ink, Boolean(sheet), settings.lockAlpha);
      }
    };
    set(x, y);
    if (settings.mirrorX) set(width - 1 - x, y);
    if (settings.mirrorY) {
      set(x, height - 1 - y);
      if (settings.mirrorX) set(width - 1 - x, height - 1 - y);
    }
  }

  function contains(x, y) {
    return !selection || (x >= selection.x && x < selection.x + selection.width && y >= selection.y && y < selection.y + selection.height);
  }
  const selectedIndex = index => contains(index % rect.width, Math.floor(index / rect.width));

  function paint(from, to, { tool, ink }) {
    const brush = (x, y) => {
      const radius = (settings.brush - 1) / 2;
      for (let dy = -radius; dy <= radius; dy++) {
        for (let dx = -radius; dx <= radius; dx++) {
          if (tool !== "Dither" || ((x + dx + y + dy) & 1) === 0) plot(x + dx, y + dy, ink);
        }
      }
    };
    if (tool === "Rectangle" || tool === "Ellipse") {
      const left = Math.min(from[0], to[0]), right = Math.max(from[0], to[0]);
      const top = Math.min(from[1], to[1]), bottom = Math.max(from[1], to[1]);
      const centerX = (left + right) / 2, centerY = (top + bottom) / 2;
      const radiusX = (right - left + 1) / 2, radiusY = (bottom - top + 1) / 2;
      const inside = tool === "Ellipse"
        ? (x, y) => ((x - centerX) / radiusX) ** 2 + ((y - centerY) / radiusY) ** 2 <= 1
        : (x, y) => x >= left && x <= right && y >= top && y <= bottom;
      for (let y = top; y <= bottom; y++) {
        for (let x = left; x <= right; x++) {
          if (inside(x, y) && (settings.solid || !inside(x - 1, y) || !inside(x + 1, y)
            || !inside(x, y - 1) || !inside(x, y + 1))) plot(x, y, ink);
        }
      }
    } else {
      line(...from, ...to, brush);
    }
    queuePaint();
  }

  function mark(from, to) {
    selection = { x: Math.min(from[0], to[0]), y: Math.min(from[1], to[1]),
      width: Math.abs(from[0] - to[0]) + 1, height: Math.abs(from[1] - to[1]) + 1 };
    doc.spriteSelection = { id: selectionId(), bounds: selection };
    updateSelectionActions();
    queuePaint();
  }

  function clearRegion(copy = false) {
    if (!selection) return;
    if (copy) copyRegion();
    const before = pixels.slice();
    const clear = sheet ? 46 : 0;
    const { x, y, width, height } = selection;
    for (let row = y; row < y + height; row++) pixels.fill(clear, row * rect.width + x, row * rect.width + x + width);
    commit(before);
    repaint();
  }

  function copyRegion() {
    if (!selection) return;
    clipboard = { ...selection, pixels: readTile({ width: rect.width, pixels }, selection) };
    updateSelectionActions();
  }

  function armPaste() {
    if (!clipboard) return;
    pasteArmed = true;
    canvas.focus({ preventScroll: true });
    updateSelectionActions();
  }

  function pasteAt(x, y) {
    const before = pixels.slice();
    const width = Math.min(clipboard.width, rect.width - x);
    const height = Math.min(clipboard.height, rect.height - y);
    for (let row = 0; row < height; row++) {
      const start = row * clipboard.width;
      pixels.set(clipboard.pixels.subarray(start, start + width), (y + row) * rect.width + x);
    }
    pasteArmed = false;
    mark([x, y], [x + width - 1, y + height - 1]);
    commit(before);
  }

  canvas.onpointerdown = event => {
    if (event.button !== 0 || stroke) return;
    canvas.focus({ preventScroll: true });
    const location = point(event);
    const [x, y] = location;
    if (pasteArmed) { pasteAt(x, y); return; }
    const tool = settings.tool;
    if (tool === "Marquee") {
      stroke = { id: event.pointerId, tool, start: location, previous: selection };
      canvas.setPointerCapture(event.pointerId);
      mark(location, location);
      return;
    }
    if (tool === "Pick") {
      const pixel = pixels[y * rect.width + x];
      if (sheet) settings.key = String.fromCharCode(pixel);
      else { settings.color = hex(pixel).slice(0, 6); settings.alpha = pixel & 255; }
      setTool("Pencil");
      paletteControls();
      return;
    }
    const clear = sheet ? ".".charCodeAt(0) : 0;
    const ink = tool === "Erase" ? clear : sheet ? settings.key.charCodeAt(0) : rgba(settings.color, settings.alpha);
    const before = pixels.slice();
    if (tool === "Fill" || tool === "Replace") {
      if (tool === "Fill") fill(pixels, rect.width, rect.height, x, y, ink, selectedIndex);
      else {
        if (!contains(x, y)) return;
        const target = pixels[y * rect.width + x];
        pixels = pixels.map((pixel, index) => pixel === target && selectedIndex(index) ? ink : pixel);
      }
      if (settings.lockAlpha) pixels = pixels.map((pixel, index) => paintInk(before[index], pixel, Boolean(sheet), true));
      queuePaint();
      commit(before);
      return;
    }
    stroke = { id: event.pointerId, before, tool, ink, start: location, last: location };
    canvas.setPointerCapture(event.pointerId);
    paint(location, location, stroke);
  };

  canvas.onpointermove = event => {
    const at = point(event);
    position.textContent = (at[0] + rect.x) + ", " + (at[1] + rect.y) + " px";
    if (!stroke || stroke.id !== event.pointerId) return;
    if (stroke.tool === "Marquee") { mark(stroke.start, at); return; }
    const shape = shapeTools.has(stroke.tool);
    if (shape) pixels.set(stroke.before);
    paint(shape ? stroke.start : stroke.last, at, stroke);
    stroke.last = at;
  };

  canvas.onpointerup = event => {
    if (stroke?.id !== event.pointerId) return;
    canvas.onpointermove(event);
    const { before } = stroke;
    stroke = undefined;
    if (before) commit(before);
  };

  canvas.onlostpointercapture = () => {
    if (!stroke) return;
    if (stroke.before) pixels.set(stroke.before);
    else {
      selection = stroke.previous;
      doc.spriteSelection = { id: selectionId(), bounds: selection };
      updateSelectionActions();
    }
    stroke = undefined;
    queuePaint();
  };

  canvas.onkeydown = event => {
    if (stroke || event.altKey) return;
    if (event.ctrlKey || event.metaKey) {
      const action = { c: copyRegion, x: () => clearRegion(true), v: armPaste }[event.key.toLowerCase()];
      if (action) { event.preventDefault(); action(); }
      return;
    }
    if (event.key === "Delete") { event.preventDefault(); clearRegion(); return; }
    if (event.key === "Escape") {
      selection = undefined;
      delete doc.spriteSelection;
      pasteArmed = false;
      updateSelectionActions();
      repaint();
      return;
    }
    const selected = tools.find(([, key]) => key === event.key.toUpperCase());
    if (selected) { event.preventDefault(); setTool(selected[0]); }
  };

  function flip(horizontal) {
    const before = pixels.slice();
    for (let y = 0; y < rect.height; y++) {
      for (let x = 0; x < rect.width; x++) {
        const from = horizontal ? y * rect.width + rect.width - x - 1 : (rect.height - y - 1) * rect.width + x;
        pixels[y * rect.width + x] = before[from];
      }
    }
    commit(before);
    repaint();
  }

  function rotate() {
    const before = pixels.slice();
    const size = rect.width;
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) pixels[x * size + size - 1 - y] = before[y * size + x];
    }
    commit(before);
    repaint();
  }

  function outline() {
    const before = pixels.slice();
    const clear = sheet ? 46 : 0;
    const ink = sheet ? settings.key.charCodeAt(0) : rgba(settings.color, settings.alpha);
    const { width } = rect;
    const solid = sheet ? pixel => pixel !== undefined && pixel !== clear : pixel => Boolean(pixel & 255);
    for (let i = 0; i < before.length; i++) {
      const x = i % width;
      if (!solid(before[i]) && ((x > 0 && solid(before[i - 1])) || (x < width - 1 && solid(before[i + 1]))
        || solid(before[i - width]) || solid(before[i + width]))) pixels[i] = ink;
    }
    commit(before);
    repaint();
  }

  function colorLab() {
    const before = pixels.slice();
    const base = resolve ? before.map(resolve) : before;
    const original = doc.text;
    const previews = tintPreviews("Adjusted", "Original");
    const hue = hueControl(0, queue);
    const filter = el("select", { onchange: queue }, ["None", "Grayscale", "Sepia", "Invert", "Posterize"]
      .map(name => choice(name, name)));
    const saturation = el("input", { type: "range", min: 0, max: 200, value: 100, oninput: queue });
    const contrast = el("input", { type: "range", min: 50, max: 170, value: 100, oninput: queue });
    const brightness = el("input", { type: "range", min: -100, max: 100, value: 0, oninput: queue });
    const gamma = el("input", { type: "range", min: 25, max: 300, value: 100, oninput: queue });
    const strength = el("input", { type: "range", min: 0, max: 100, value: 85, oninput: queue });
    const modes = [["coherent", "Coherent shift"], ["ramp", "Ramp remix"], ["accents", "Accent shuffle"],
      ["material", "Material variation"], ["mood", "Mood variation"], ["wild", "Wild variation"]];
    const mode = el("select", { onchange: queue }, modes.map(([id, name]) => choice(id, name)));
    const seed = number(1, 0, 2147483647);
    seed.onchange = queue;
    const slider = (value, min, max) => el("input", { type: "range", min, max, value, oninput: queue });
    const range = slider(100, 10, 180), shadow = slider(-28, -90, 90);
    const highlight = slider(32, -90, 90), chroma = slider(32, 4, 40);
    const neutrals = el("input", { type: "checkbox", checked: true, onchange: queue });
    const sheetPreviews = el("div", { className: "sprite-variant-sheet", "aria-label": "Palette variants" });
    const scope = el("select", { onchange: queue }, [selection && !sheet?.masks ? choice("selection", "Selected pixels") : null,
      choice("all", sheet?.masks ? "Shared tool material ramp" : "Whole sprite")]);
    scope.value = selection && !sheet?.masks ? "selection" : "all";
    const colors = swatches?.filter(swatch => !swatch.shared) ?? [...new Set(before)].filter(pixel => pixel & 255)
      .slice(0, 64).map(pixel => ({ key: hex(pixel), rgb: hex(pixel).slice(0, 6), alpha: pixel & 255 }));
    const locked = new Set();
    const locks = el("div", { className: "sprite-palette-locks" },
      colors.map(({ key, rgb, alpha }) => {
        const toggle = button(sheet ? key : "●", () => {
          if (locked.has(key)) locked.delete(key); else locked.add(key);
          toggle.setAttribute("aria-pressed", String(locked.has(key)));
          toggle.textContent = locked.has(key) ? "✓" : sheet ? key : "●";
          queue();
        }, "Lock #" + rgb.toUpperCase() + (alpha !== undefined && alpha < 255 ? " · alpha " + alpha : ""), "sprite-swatch");
        toggle.setAttribute("aria-label", toggle.title);
        toggle.setAttribute("aria-pressed", "false");
        toggle.style.backgroundColor = "#" + rgb;
        return toggle;
      }));
    let selected = -1, pinned, offsets = [], scheduled, adjusted, paletteResult;
    function queue() { scheduled ??= requestAnimationFrame(draw); }
    function draw() {
      if (scheduled !== undefined) cancelAnimationFrame(scheduled);
      scheduled = undefined;
      const options = { hue: +hue.input.value, saturation: saturation.value / 100, contrast: contrast.value / 100,
        brightness: brightness.value / 100, gamma: gamma.value / 100, filter: filter.value };
      const variation = { mode: mode.value, strength: strength.value / 100, shadow: +shadow.value,
        highlight: +highlight.value, chromaLimit: chroma.value / 100, protectNeutrals: neutrals.checked };
      const scoped = index => scope.value !== "selection" || selectedIndex(index);
      function candidate(offset) {
        const memo = new Map();
        const transform = pixel => {
          if (!sheet && locked.has(hex(pixel))) return pixel;
          if (memo.has(pixel)) return memo.get(pixel);
          const color = adjustColor(pixel, options);
          const result = offset === undefined ? color : varyColor(color, { ...variation, offset });
          memo.set(pixel, result);
          return result;
        };
        if (!sheet) return { colors: [], pixels: before.map((pixel, i) => scoped(i) ? transform(pixel) : pixel) };
        const swatchResults = colors.map(swatch => {
          const color = rgba(swatch.rgb);
          return { ...swatch, result: locked.has(swatch.key) ? color : transform(color) };
        });
        const mapped = new Map(swatchResults.map(({ key, result }) => [key.charCodeAt(0), result]));
        return { colors: swatchResults, pixels: before.map((pixel, i) => scoped(i) ? mapped.get(pixel) ?? base[i] : base[i]) };
      }
      offsets = paletteCandidates(getNumber(seed, 1), pinned === undefined ? +range.value : +range.value / 4, pinned ?? 0);
      if (pinned !== undefined) offsets[0] = pinned;
      const variants = offsets.map(candidate);
      const current = variants[selected] ?? candidate();
      adjusted = current.pixels;
      paletteResult = current.colors;
      previews.pixels(base, adjusted, rect.width, rect.height);
      sheetPreviews.replaceChildren(...variants.map(({ pixels }, i) => {
        const icon = el("canvas", { "aria-hidden": "true" });
        canvasPixels(icon, pixels, rect.width, rect.height);
        const control = button("", () => {
          selected = i; draw(); sheetPreviews.children[i]?.focus();
        }, "Preview variant " + (i + 1), "sprite-variant");
        control.setAttribute("aria-pressed", String(i === selected));
        control.setAttribute("aria-label", control.title);
        control.append(icon, el("span", {}, (i === 0 && pinned !== undefined ? "Pinned" : "#" + (i + 1)) + " · " + offsets[i] + "°"));
        return control;
      }));
    }
    const reroll = button("New Variants", () => { seed.value = (getNumber(seed, 0) + 1) % 2147483647; queue(); });
    const pin = button("Pin Variant", () => { if (selected >= 0) { pinned = offsets[selected]; selected = 0; draw(); } });
    const reset = button("Reset", () => {
      selected = -1;
      pinned = undefined;
      locked.clear();
      for (const [i, lock] of [...locks.children].entries()) {
        lock.setAttribute("aria-pressed", "false");
        lock.textContent = sheet ? colors[i].key : "●";
      }
      hue.input.value = brightness.value = 0;
      saturation.value = contrast.value = gamma.value = 100;
      strength.value = 85;
      mode.value = "coherent";
      seed.value = 1;
      range.value = 100;
      shadow.value = -28;
      highlight.value = 32;
      chroma.value = 32;
      neutrals.checked = true;
      filter.value = "None";
      hue.input.dispatchEvent(new Event("input"));
      queue();
    });
    draw();
    ui.dialog("Color lab", [el("div", { className: "idea-fields sprite-color-lab" },
      label("Apply to", scope), previews.view, hue.view,
      label("Filter", filter), label("Saturation", saturation), label("Brightness", brightness), label("Contrast", contrast), label("Gamma", gamma),
      el("details", { className: "sprite-whackifier" }, el("summary", {}, "Palette Whackifier"),
        el("small", { className: "muted" }, "Palette algorithm 1 · Reproducible from seed and settings"),
        sheetPreviews, el("div", { className: "sprite-transform" }, reroll, pin, button("Clear Variant", () => { selected = -1; pinned = undefined; queue(); })),
        el("div", { className: "sprite-variant-controls" },
          label("Mode", mode), label("Seed", seed), label("Strength", strength), label("Hue range", range),
          label("Shadow bias", shadow), label("Highlight bias", highlight), label("Chroma limit", chroma), label("Protect neutrals", neutrals)),
        locks.childElementCount ? el("div", {}, el("span", {}, "Lock colors"), locks) : null,
        !sheet ? el("small", { className: "muted" }, "Locks list up to 64 opaque colors from the tile.") : null,
        sheet && !sheet.masks ? el("small", { className: "muted" }, "Shared outline and transparency are protected.") : null),
      reset)],
    "Apply to draft", () => {
      if (!stage.isConnected || doc.text !== original) throw new Error("The sprite draft changed. Reopen Color Lab.");
      draw();
      if (sheet) {
        if (scope.value === "selection") return ui.update(applySelectedColors(doc.text, entry, before, adjusted, resolve));
        const text = paletteResult.reduce((draft, { offset, rgb, result }) => result === rgba(rgb)
          ? draft : patch(draft, offset, offset + 6, hex(result).slice(0, 6)), doc.text);
        return ui.update(text);
      }
      pixels.set(adjusted);
      commit(before);
      repaint();
      return true;
    });
  }

  async function copyWithTint() {
    try {
      const previews = tintPreviews("Tinted copy");
      const hue = hueControl(40, () => draw());
      let draw, apply, fields, caption;
      if (sheet) {
        const paths = ["items", "resources", "machines", "effects"].map(name => "assets/icons/" + name + "16.txt");
        const sheets = await Promise.all(paths.map(async path =>
          path === doc.path ? sheet : readSheet((await ui.load(path)).text, path)));
        if (!stage.isConnected) return;
        const sources = sheets.flatMap((source, i) => source.entries.filter(row => row !== entry)
          .map(row => ({ row, group: paths[i].split("/").at(-1).replace("16.txt", "") })));
        const source = el("select", {}, paths.map((_, i) => {
          const group = paths[i].split("/").at(-1).replace("16.txt", "");
          return el("optgroup", { label: group }, sources.flatMap((item, index) => item.group === group
            ? [choice(String(index), item.row.name.replaceAll("_", " "))] : []));
        }));
        source.value = String(Math.max(0, sources.findIndex(item => item.group === doc.path.split("/").at(-1).replace("16.txt", ""))));
        const chosen = () => sources[Number(source.value)].row;
        draw = () => previews.sprite(chosen(), Number(hue.input.value), entry.palette[0]);
        source.onchange = draw;
        fields = label("Copy from", source);
        caption = "Replace " + entry.name.replaceAll("_", " ") + " with the tinted copy.";
        const original = doc.text;
        apply = () => {
          if (doc.text !== original) throw new Error("The sprite sheet changed. Reopen the tint tool.");
          return ui.update(tintedSprite(doc.text, entry, chosen(), Number(hue.input.value)));
        };
      } else {
        const columns = Math.floor((bitmap.width - rect.width) / settings.tileWidth) + 1;
        const rows = Math.floor((bitmap.height - rect.height) / settings.tileHeight) + 1;
        if (columns * rows < 2) throw new Error("Set the tile size to fit at least two tiles in this image.");
        const column = number(settings.column === 0 && columns > 1 ? 2 : 1, 1, columns);
        const row = number(columns === 1 && settings.row === 0 ? 2 : 1, 1, rows);
        column.required = row.required = true;
        let tinted;
        draw = () => {
          if (!column.validity.valid || !row.validity.valid || !column.value || !row.value) return;
          const source = readTile(bitmap, { ...rect, x: (Number(column.value) - 1) * settings.tileWidth,
            y: (Number(row.value) - 1) * settings.tileHeight });
          tinted = source.map(pixel => shiftHue(pixel, Number(hue.input.value)));
          previews.pixels(source, tinted, rect.width, rect.height);
        };
        column.oninput = row.oninput = draw;
        fields = el("div", { className: "item-nutrition" }, label("Source column", column), label("Source row", row));
        caption = "Copy into column " + (settings.column + 1) + ", row " + (settings.row + 1) + ".";
        apply = () => {
          if (+column.value === settings.column + 1 && +row.value === settings.row + 1) throw new Error("Choose another source tile");
          const before = pixels.slice();
          pixels.set(tinted);
          commit(before);
          repaint();
          return true;
        };
      }
      draw();
      ui.dialog("Copy with tint", [el("div", { className: "idea-fields" },
        el("p", { className: "muted" }, caption), fields, hue.view, previews.view)], "Copy to draft", apply);
    } catch (error) { ui.note(error.message, true); }
  }

  const cutButton = button("Cut", () => clearRegion(true), "Cut the marquee (Ctrl+X)");
  const copyButton = button("Copy", copyRegion, "Copy the marquee (Ctrl+C)");
  const pasteButton = button("Paste", armPaste, "Click the destination to paste (Ctrl+V)");
  const selectionHint = el("span", { className: "muted", role: "status" });
  function updateSelectionActions() {
    cutButton.disabled = copyButton.disabled = !selection;
    pasteButton.disabled = !clipboard;
    selectionHint.textContent = pasteArmed ? "Click the top-left of the paste destination."
      : selection ? selection.width + " × " + selection.height + " selected" : "Drag with Marquee to select pixels.";
  }
  updateSelectionActions();

  const brush = el("select", { onchange: event => { settings.brush = +event.target.value; } },
    [1, 3, 5].map(size => choice(String(size), size + " px")));
  brush.value = String(settings.brush);
  const toggle = (title, key) => label(title, el("input", { type: "checkbox", checked: settings[key],
    onchange: event => { settings[key] = event.target.checked; } }));
  rotateButton = button("Rotate 90°", rotate, "Rotate a square sprite clockwise");
  rotateButton.disabled = rect.width !== rect.height;
  bar.append(el("div", { className: "chips", "aria-label": "Paint tools" }, toolButtons),
    el("div", { className: "sprite-view-options" }, label("Zoom", zoomControl), label("Grid", grid),
      label("Brush", brush), toggle("Mirror X", "mirrorX"), toggle("Mirror Y", "mirrorY"),
      toggle("Solid shapes", "solid"), toggle("Lock paint alpha", "lockAlpha")));
  setTool(settings.tool);
  paletteControls();
  requestAnimationFrame(() => { if (stage.isConnected) updateZoom(); });
  const properties = el("aside", { className: "sprite-properties", "aria-label": "Sprite properties" },
    el("section", {}, el("h3", {}, sheet ? "Selection" : "Atlas tile"), selector),
    el("section", {}, el("h3", {}, "Palette"), palette),
    el("section", {}, el("h3", {}, "Clipboard"), el("div", { className: "sprite-transform" },
      cutButton, copyButton, pasteButton), selectionHint),
    el("section", {}, el("h3", {}, "Preview"),
      el("div", { className: "sprite-previews" }, preview, assembled),
      label("Actual size", el("input", { type: "checkbox", checked: settings.actualSize, onchange: event => {
        settings.actualSize = event.target.checked; repaint();
      } })),
      el("div", { className: "sprite-transform" },
        button("Color lab", colorLab, "Hue, filters and Palette Whackifier", "primary"),
        !sheet?.masks ? button("Copy with tint", copyWithTint, "Copy and recolor another sprite") : null,
        button("Flip horizontal", () => flip(true), "Mirror the current tile horizontally"),
        button("Flip vertical", () => flip(false), "Mirror the current tile vertically"),
        rotateButton, button("Outline", outline, "Outline opaque pixels using the active color"))));
  const nativePreview = sheet?.masks ? button("Native preview", async () => {
    if (doc.text !== doc.original) return ui.note("Save mask edits before opening the native preview.", true);
    nativePreview.disabled = true;
    nativePreview.textContent = "Rendering native…";
    try {
      const result = await ui.api("/api/render-saved-tool-art", undefined, {});
      if (result.sourceSha256 !== doc.version) throw new Error("The mask file changed. Reload it before previewing again.");
      const bytes = Uint8Array.from(atob(result.png), value => value.charCodeAt(0));
      const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      ui.dialog("Native tool preview", [el("p", {}, result.scope),
        el("p", {}, "The first four rows show tools in stone, copper, iron, and steel. The last two show copper parts and clay molds."),
        el("div", { style: "overflow:auto;max-height:65vh" }, el("img", { src: url,
          alt: "Saved tool masks rendered by the native game composer", style: "width:" + result.width * 4 + "px;max-width:none;image-rendering:pixelated" })),
        el("p", { className: "muted" }, "Evidence: " + result.output)], "Done", () => { URL.revokeObjectURL(url); return true; });
    } catch (error) { ui.note(error.message, true); }
    finally { nativePreview.disabled = false; nativePreview.textContent = "Native preview"; }
  }, "Render the saved masks with the game's native tool composer") : null;
  const toolsHeader = el("header", { className: "sprite-studio-header" }, button("Sprite Library", ui.sprites), nativePreview,
    button("Find Usages", async () => {
      try {
        const uses = await ui.api("/api/usages", doc.path);
        ui.dialog("Sprite sheet usages", uses.length ? uses.slice(0, 24).map(use => button(use.path + " · " + use.line, () => {
          document.querySelector("dialog[open]")?.close();
          ui.open(use.path, undefined, use.line);
        })) : [el("p", {}, "No direct loader references found.")], "Done", () => true);
      } catch (error) { ui.note(error.message, true); }
    }),
    button("Foliage wind guide", () => ui.open("docs/axiom-notes/sprite-foliage-wind.md"), "Open sprite wind reference and Axiom acceptance brief"),
    button("Normal map lab", () => openNormalLab(native, entry?.name ?? "sprite")),
    button("Export PNG", () => native.toBlob(blob => download(blob, (entry?.name ?? "sprite") + ".png"))),
    sheet ? button("Export Source", () => download(new Blob([doc.text], { type: "text/plain" }), doc.path.split("/").at(-1))) : null);
  return el("div", {}, toolsHeader, doc.recovered && doc.text !== doc.original
    ? el("p", { className: "sprite-recovery", role: "status" }, doc.recovered) : null,
    el("div", { className: "sprite-layout" },
      el("div", { className: "sprite-work" }, bar, stage, el("div", { className: "sprite-footer" }, info, position)),
      properties));
}

