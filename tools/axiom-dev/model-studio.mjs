import { el, button } from "./inspector.mjs";
import { parseVoxel, setVoxel, changeVoxelPalette } from "./voxel-model.mjs";
import { voxelFaces, orbitModel, glbFaces, modelBounds, paintModel, rotateX, rotateY, subtract } from "./model-view.mjs";

const caption = text => el("p", { className: "muted" }, text);
const label = (name, control) => el("label", {}, name, control);
const canvas = label => el("canvas", { className: "studio-canvas", width: 700, height: 450, "aria-label": label });
export async function loadMesh(ctx, path) {
  const blob = await ctx.api("/api/model", path, undefined, true);
  return glbFaces(await blob.arrayBuffer());
}

export async function meshPreview(ctx, spec) {
  const scene = canvas("3D model preview");
  const faces = await loadMesh(ctx, spec.path);
  const bounds = modelBounds(faces);
  orbitModel(scene, () => faces, () => bounds);
  return el("div", { className: "studio-preview" }, scene,
    caption("Drag to orbit · scroll to zoom · static geometry preview"),
    button("Open asset", () => ctx.open(spec.path)));
}

export async function voxelThumbnail(ctx, spec) {
  const model = parseVoxel((await ctx.load(spec.path)).text);
  const image = el("canvas", { width: 64, height: 64, className: "catalog-art", "aria-hidden": "true" });
  const center = model.size.map(n => n/2);
  const scale = 38 / Math.max(...model.size);
  paintModel(image, voxelFaces(model), {
    transform: point => rotateX(rotateY(subtract(point, center), -.65), -.46),
    project: point => [32 + point[0]*scale, 32 - point[1]*scale],
  });
  return image;
}

export function voxelStudio(ctx, doc, changed) {
  let model = parseVoxel(doc.text);
  let y = [...model.layers.keys()][0] ?? 0;
  let ink = [...model.palette.keys()][0];
  let erase = false, dragging = false;
  let faces = voxelFaces(model);
  const scene = canvas("Voxel model preview");
  const frame = { center: model.size.map(value => value/2), radius: Math.max(...model.size)*.65 };
  const repaint = orbitModel(scene, () => faces, () => frame);
  const layer = el("select", { onchange: event => { y = Number(event.target.value); showLayer(); } },
    Array.from({ length: model.size[1] }, (_, i) => el("option", { value: String(i) }, "Y " + i)));
  layer.value = String(y);
  const grid = el("div", { className: "voxel-grid", "aria-label": "Voxel layer" });
  grid.style.gridTemplateColumns = "repeat(" + model.size[0] + ", minmax(0, 1fr))";
  const palette = el("div", { className: "voxel-palette" });
  const info = el("p", { className: "muted", role: "status" });
  const rubber = button("Erase", () => { erase = !erase; rubber.setAttribute("aria-pressed", String(erase)); });
  const live = button("Apply to game", async () => {
    live.disabled = true;
    try {
      if(!/^assets\/models\/factory\/[\w-]+\.vox$/.test(doc.path))throw new Error("Live replacement supports factory voxel assets");
      const current = await ctx.api("/api/engine", undefined, {op:"inspect",sections:["models"]});
      const result = await ctx.api("/api/engine", undefined, {op:"model",revision:current.revision,
        asset:doc.path.split("/").at(-1).replace(/\.vox$/,""),text:doc.text});
      info.textContent = result.data.voxels + " voxels · Live preview · Undo in Engine workbench";
    } catch(error) {ctx.note(error.message,true);} finally {live.disabled=false;}
  }, "Replace the loaded model immediately; source remains a draft");
  rubber.setAttribute("aria-pressed", "false");

  function refreshCell(control, x, z) {
    const key = model.cells.get([x,y,z].join(","));
    control.style.backgroundColor = key ? "#" + model.palette.get(key).hex : "";
    control.setAttribute("aria-label", "X " + x + " Y " + y + " Z " + z + (key ? " · " + key : " · empty"));
  }

  function commit(text) {
    try {
      const updated = parseVoxel(text);
      if (!ctx.edit(doc, text)) return false;
      model = updated; faces = voxelFaces(model);
      info.textContent = model.cells.size + " voxels";
      repaint(); changed();
      return true;
    } catch (error) { ctx.note(error.message, true); return false; }
  }

  function showLayer() {
    const buttons = [];
    for (let z = 0; z < model.size[2]; z++) for (let x = 0; x < model.size[0]; x++) {
      const cell = button("", event => { if (event.detail === 0) paint(event); }, "", "voxel-cell");
      function paint(event) {
        const next = erase || event.shiftKey || event.button === 2 ? "." : ink;
        if ((model.cells.get([x,y,z].join(",")) ?? ".") === next) return;
        if (commit(setVoxel(model, x, y, z, next))) refreshCell(cell, x, z);
      }
      cell.onpointerdown = event => {
        if (event.button !== 0 && event.button !== 2) return;
        dragging = event.button === 0;
        paint(event);
      };
      cell.onpointerenter = event => { if (dragging && event.buttons === 1) paint(event); };
      cell.onpointerup = () => { dragging = false; };
      cell.oncontextmenu = event => event.preventDefault();
      refreshCell(cell, x, z);
      buttons.push(cell);
    }
    grid.replaceChildren(...buttons);
  }

  function showPalette() {
    const swatches = [...model.palette].map(([key, { hex }]) => {
      const color = button(key, () => { ink = key; erase = false; rubber.setAttribute("aria-pressed", "false"); showPalette(); },
        "Paint with " + key + " · #" + hex, "voxel-swatch" + (key === ink ? " active" : ""));
      color.style.backgroundColor = "#" + hex;
      color.setAttribute("aria-pressed", String(key === ink));
      return color;
    });
    const material = model.palette.get(ink);
    const color = el("input", { type: "color", value: "#" + material.hex });
    const glow = el("input", { type: "number", min: 0, max: 64, step: .05, value: String(material.glow) });
    const editColor = () => {
      if (!glow.validity.valid || glow.value === "") return;
      if (commit(changeVoxelPalette(model, ink, color.value.slice(1), glow.value))) { showPalette(); showLayer(); }
    };
    color.onchange = glow.onchange = editColor;
    palette.replaceChildren(el("div", { className: "voxel-swatches" }, swatches),
      label("Color", color), label("Glow", glow), button("Add color", () => {
        const key = [..."ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"].find(key => !model.palette.has(key));
        if (!key) return ctx.note("The palette is full", true);
        if (commit(changeVoxelPalette(model, key, material.hex, 0))) { ink = key; showPalette(); }
      }));
  }

  showLayer(); showPalette();
  info.textContent = model.cells.size + " voxels";
  return el("div", { className: "voxel-studio" },
    el("div", { className: "studio-preview" }, scene, caption("Drag to orbit · scroll to zoom"), live, info),
    el("div", { className: "voxel-controls" }, el("div", { className: "studio-tools" }, label("Layer", layer), rubber),
      grid, caption("Paint cells · right-click or Shift-click to erase"), palette));
}

