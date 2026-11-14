import { el } from "./inspector.mjs";
import { field, textOf } from "./idea-data.mjs";
import { readSheet, cells, rgba, masksPath } from "./sprite-sheet.mjs";
import { toolAppearance } from "./sprite-composition.mjs";
import { canvasPixels } from "./pixel-grid.mjs";
import { foodTemplates, foodIconId } from "./food-item.mjs";

const glyphs = {
  Recipes: "M4 6h16v12H4z M9 6v12m6-12v12M4 10h16M4 14h16",
  "Recipe Tree": "M4 5h6v4H4z M10 7h4v10m0-6h3m-3 6h3m0-8h4v4h-4z m0 6h4v4h-4z",
  Items: "M4 8l8-4 8 4v9l-8 4-8-4z M4 8l8 4 8-4m-8 4v9M8 6l8 4v3",
  Creatures: "M4 9l3-5 4 4h2l4-4 3 5v7l-4 4H8l-4-4z M8 12h2m4 0h2m-6 4h4",
  Locations: "M3 19l6-11 4 6 3-4 5 9z M14 6h4",
  Biomes: "M4 18c2-8 8-12 16-13-1 9-5 15-13 15H4z M4 21 15 10m-7 7h6",
  Animals: "M6 9c-2-3-2-5 0-5s3 2 3 4m6 0c0-2 1-4 3-4s2 2 0 5M3 13c0-4 4-3 5 0s-3 5-5 0m13 0c1-3 5-4 5 0s-4 3-5 0m-9 6c0-3 3-6 5-6s5 3 5 6-3 1-5 1-5 2-5-1",
  Deposits: "M4 9l8-6 8 6-4 11H8z M4 9h16m-12 0 4 11 4-11-4-6z",
  Models: "M12 3l8 4.5v9L12 21l-8-4.5v-9z M12 12l8-4.5m-8 4.5L4 7.5m8 4.5v9",
  Sprites: "M3 4h18v16H3z M3 16l5-5 4 4 3-3 6 5M15 8h2v2h-2z",
  "Hand poses": "M7 12V6a1.5 1.5 0 013 0v5-7a1.5 1.5 0 013 0v7-5a1.5 1.5 0 013 0v6-3a1.5 1.5 0 013 0v6c0 4-2.5 6-6.5 6H11c-2.2 0-3.6-1-5-3l-3-4a1.5 1.5 0 012.4-1.8L7 14",
  "All files": "M5 3h10l4 4v14H5z M14 3v5h5M8 12h8m-8 4h8",
  Flyer: "M12 12c-2-5-5-7-9-6 0 6 3 9 8 9l1 4 1-4c5 0 8-3 8-9-4-1-7 1-9 6z",
  Grazer: "M5 4v6l4 3m10-9v6l-4 3M8 13l4-3 4 3v4l-4 3-4-3z",
  Defender: "M12 3l7 3v7c0 4-3 6-7 8-4-2-7-4-7-8V6z M9 12l2 2 4-4",
  Rooted: "M12 21v-8M12 14C5 14 3 9 4 5c4-1 8 3 8 7 0-4 4-8 8-7 1 4-1 9-8 9",
  Anvil: "M2 6h20l-3 4h-4v4l4 2v3H6v-3l4-2v-4H6z",
  Bed: "M3 5v14m0-3h18v3m0-3v-4H3m2-2V7h5v5m0-2h8l3 2",
  Workbench: "M3 7h18v4H3z M6 11v8m12-8v8M6 16h12",
  Bookcase: "M4 3h16v18H4z M4 12h16M8 6v6m4-5v5m3-6 2 6M8 15v6m4-6v6m3-5 2 5",
  Weapon: "M6 18 18 6m-1 5 3-7-7 3-7 8m-2-2 7 7m-6-3-3 3 2 2 3-3",
  Tool: "M14 6l4-3 2 2-2 5-4 2-8 9-3-3 8-8 1-4 3-2z",
  Cast: "M4 6h16v12H4z M8 10h8v4H8z",
  Armor: "M8 4 3 7l2 5 3-1v9h8v-9l3 1 2-5-5-3-4 3z",
};

export function catalogIcon(section) {
  const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  icon.setAttribute("viewBox", "0 0 24 24");
  icon.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(icon.namespaceURI, "path");
  path.setAttribute("d", glyphs[section] ?? glyphs["All files"]);
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.55");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  icon.append(path);
  return icon;
}

export async function recipeArtwork(ctx) {
  let masks;
  try { masks = readSheet((await ctx.load(masksPath)).text, masksPath); } catch { masks = null; }
  const composed = code => masks ? toolAppearance(code, masks, masksPath) : null;
  const resources = "assets/icons/resources16.txt", items = "assets/icons/items16.txt", machines = "assets/icons/machines16.txt";
  const sheets = new Map(await Promise.all([resources, items, machines].map(async path => {
    try {
      const { text } = await ctx.load(path);
      return [path, new Map(readSheet(text, path).entries.map((entry, index) =>
        [Number(entry.id.split(" · ")[0]), { entry, path, index }]))];
    } catch { return [path, new Map()]; }
  })));
  const [machinesDoc, materials, inventory, foods] = await Promise.all([
    ctx.load("assets/machines.ron"), ctx.load("assets/materials.ron"), ctx.load("crates/app/src/world/inventory.rs"),
    ctx.load("assets/foods.ron"),
  ]);
  const blocks = new Map(ctx.tree(machinesDoc).children.map((row, i) => [textOf(field(row, "block")), i]));
  const colors = new Map(ctx.tree(materials).children.map(row => {
    const base = field(field(field(row, "render"), "color"), "base");
    return [textOf(field(row, "id")), base?.children?.map(channel => Number(textOf(channel)))];
  }));
  const foodEnum = /pub enum FoodKind\s*\{([^}]+)\}/.exec(inventory.text)?.[1] ?? "";
  const foodIds = new Map([...foodEnum.matchAll(/(\w+)\s*=\s*(\d+)/g)].map(([, kind, id]) => [kind, Number(id)]));
  const foodIcons = new Map(foodTemplates(foods.text).map(({ kind, row }) => [kind, foodIconId(row, foodIds.get(kind))]));
  const toolIds = { 1: 40, 2: 40, 6: 6, 7: 7, 8: 8, 9: 5, 10: 4, 11: 3, 18: 41 };
  const blockIds = { Torch: 2, Ladder: 37, Vines: 38, Sapling: 39, ClaimStake: 75 };
  const blockGlyphs = { Anvil: "Anvil", Bed: "Bed", Bookcase: "Bookcase", Workbench: "Workbench",
    CrystalWorkbench: "Workbench", ArtificerWorkbench: "Workbench" };
  function describe(code) {
    const [, variant, token, tier] = /^(\w+)\((\w+)(?:,(\w+))?\)$/.exec(code) ?? [];
    let path = items, id;
    if (variant === "Resource") { path = resources; id = Number(token); }
    if (variant === "Block") {
      id = blockIds[token];
      if (id === undefined) { path = machines; id = blocks.get(token); }
    }
    if (variant === "Tool") id = toolIds[token];
    if (variant === "Bucket") id = Number(token) === 0 ? 0 : 1;
    if (variant === "Food") {
      id = foodIcons.get(token);
    }
    if (variant === "Armor") {
      const slot = { Head: 0, Chest: 1, Legs: 2, Feet: 3 }[token] ?? Number(token);
      const material = { Copper: 0, Steel: 1, Lumen: 2, Kinetic: 3 }[tier];
      id = token === "Head" && tier === "Survey" ? 76 : 20 + slot * 4 + material;
    }
    return { variant, token, sprite: composed(code) ? undefined : sheets.get(path).get(id) };
  }
  function icon(code) {
    const composition = composed(code);
    if (composition?.pixels) {
      const image = el("canvas", { className: "catalog-art", "aria-hidden": "true" });
      canvasPixels(image, composition.pixels, 16, 16);
      return image;
    }
    const { variant, token, sprite } = describe(code);
    const entry = sprite?.entry;
    if (!entry) {
      if (variant === "Block") {
        const shape = blockGlyphs[token];
        return shape ? catalogIcon(shape) : cubeIcon(colors.get(token));
      }
      return glyphs[variant] ? catalogIcon(variant) : null;
    }
    const map = new Map(entry.palette.map(({ key, rgb }) => [key.charCodeAt(0), rgba(rgb)]));
    const image = el("canvas", { className: "catalog-art", "aria-hidden": "true" });
    canvasPixels(image, cells(entry), 16, 16, value => map.get(value) ?? 0);
    return image;
  }
  return { icon, sprite: code => describe(code).sprite, parts: code => composed(code)?.parts ?? [] };
}

function cubeIcon(color) {
  if (!color?.length || color.some(channel => !Number.isFinite(channel))) return catalogIcon("Models");
  const image = el("canvas", { className: "catalog-art", "aria-hidden": "true" });
  const pixels = Uint32Array.from({ length: 196 }, (_, i) => i);
  canvasPixels(image, pixels, 14, 14, i => {
    const x = i % 14, y = Math.floor(i / 14);
    const slope = Math.abs(x + 0.5 - 7) * 0.5;
    if (y + 0.5 < slope || y + 0.5 >= 14 - slope) return 0;
    const face = y + 0.5 < 7 - slope ? 1.18 : x < 7 ? 0.92 : 0.74;
    const texture = 0.95 + ((x * 13 + y * 7) % 11) * 0.012;
    const [r, g, b] = color.map(channel => Math.round(Math.min(255, Math.max(18, channel * face * texture * 255))));
    return (r << 24) | (g << 16) | (b << 8) | 255;
  });
  return image;
}

