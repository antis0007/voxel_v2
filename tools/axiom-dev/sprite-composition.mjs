import { cells, rgba } from "./sprite-sheet.mjs";

export function composeMaterials(sheet, tool, materials) {
  const pixels = new Uint32Array(256);
  for (const [i, name] of tool.layers.entries()) {
    const mask = sheet.entries.find(entry => entry.name === name);
    const material = materials[tool.slots[i]] ?? "stone";
    const ramp = sheet.ramps.find(row => row.name === material) ?? sheet.ramps.find(row => row.name === "stone");
    if (!mask || !ramp) return null;
    cells(mask).forEach((key, index) => {
      if (key !== 46) pixels[index] = rgba(ramp.palette[key - 49].rgb);
    });
  }
  const silhouette = pixels.slice();
  silhouette.forEach((pixel, i) => {
    if (!pixel && ((i % 16 > 0 && silhouette[i - 1]) || (i % 16 < 15 && silhouette[i + 1])
      || silhouette[i - 16] || silhouette[i + 16])) pixels[i] = rgba("1a120e");
  });
  return pixels;
}

export function toolAppearance(code, sheet, path) {
  const [, variant, token] = /^(Tool|Weapon)\((\d+)\)$/.exec(code) ?? [];
  const tier = Number(token);
  const schema = variant === "Tool" && tier >= 6 && tier <= 8 ? "pickaxe" : variant === "Weapon" ? "sword" : null;
  if (!schema) return null;
  const tool = sheet.tools.find(tool => tool.name === schema);
  if (!tool) return { schema, parts: [], pixels: null };
  const materials = variant === "Tool" ? [["wood", "stone", "iron"][tier - 6], "wood", "wood"]
    : [{ 1: "copper", 2: "steel", 3: "cinder", 4: "crystal", 5: "cobalt" }[tier] ?? "iron", "iron", "wood"];
  const parts = tool.layers.map(name => {
    const index = sheet.entries.findIndex(entry => entry.name === name);
    return { path, index, entry: sheet.entries[index] };
  }).filter(part => part.entry);
  return { schema, parts, pixels: composeMaterials(sheet, tool, materials) };
}
