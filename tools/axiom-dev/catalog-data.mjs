import { address, displayKey } from "./targets.mjs";
import { field, textOf, recipeItems, spawnPath } from "./idea-data.mjs";
import { isSheet, readSheet } from "./sprite-sheet.mjs";
import { deposits, depositPath } from "./deposit-studio.mjs";
import { animalBody, isAnimal } from "./habitat-editor.mjs";
import { biomeColors } from "./biome-studio.mjs";
import { fabricationRows } from "./fabrication-data.mjs";

export const bestiaryPath = "assets/bestiary.ron";
export const wandPosePath = "assets/runtime/viewmodels/presentation.ron";
export const itemPosePath = "crates/app/src/mesh/models/held_item.rs";
export const editableVox = path => /^assets\/models\/factory\/[^/]+\.vox$/.test(path);
export const filename = path => path.split("/").at(-1);
export const titleCase = text => displayKey(text.replace(/\.[^.]+$/, "")).replace(/(^| )\w/g, word => word.toUpperCase());

export function recipeStack(doc, node) {
  if (!node) return "";
  return doc.text.slice(node.start, node.end).replace(/\s+/g, "");
}

export function recipeRows(ctx, doc) {
  const root = ctx.tree(doc);
  const stationNames = { hand: "Hand", bench: "Workbench", crystal_tech: "Crystal tech", facility: "Workbench" };
  return root.children.flatMap((table, group) => (table.children ?? []).map((row, index) => {
    const output = field(row, "output");
    const product = recipeStack(doc, field(output, "item"));
    const ingredients = field(row, "ingredients")?.children ?? [];
    const pattern = field(row, "pattern");
    const shaped = Boolean(pattern?.children?.length);
    const keys = new Map((field(row, "key")?.children ?? []).map(pair =>
      [pair.children[0].raw.slice(1, -1), recipeStack(doc, pair.children[1])]));
    const patternSlots = shaped ? pattern.children.map(line => [...textOf(line)].map(key => keys.get(key) ?? "")) : null;
    const largePattern = Boolean(patternSlots && (patternSlots.length > 2 || patternSlots.some(line => line.length > 2)));
    const station = table.key === "hand" && largePattern ? "bench" : table.key;
    const explicitInputs = ingredients.map(pair => ({
      code: recipeStack(doc, pair.children[0]), count: Number(textOf(pair.children[1])),
    }));
    const inputs = new Map();
    for (const { code, count } of [...explicitInputs, ...(patternSlots ?? []).flat().filter(Boolean).map(code => ({ code, count: 1 }))]) {
      inputs.set(code, (inputs.get(code) ?? 0) + count);
    }
    return {
      kind: "recipe", path: doc.path, parts: address(root, [group, index]), label: textOf(field(row, "name")),
      category: stationNames[station] ?? station, gridSize: station === "hand" ? 2 : 3,
      form: shaped ? "Shaped" : "Shapeless",
      product, count: Number(textOf(field(output, "count"))), inputCodes: [...inputs.keys()], patternSlots,
      inputs: [...inputs].map(([code, count]) => ({ code, count })),
      description: textOf(field(row, "description")),
    };
  })).filter(row => row.label);
}

export function machineAssemblyRows(ctx, doc) {
  const root = ctx.tree(doc);
  return root.children.flatMap((row, index) => {
    const cost = field(row, "recipe")?.children ?? [];
    if (textOf(field(row, "structural")) === "true" ||
        cost.some(pair => Number(textOf(pair.children[1])) % 64 !== 0)) return [];
    const block = textOf(field(row, "block"));
    const label = textOf(field(row, "name"));
    if (!block || !label) return [];
    const inputs = cost.map(pair => ({ code: "Resource(" + textOf(pair.children[0]) + ")",
      count: Number(textOf(pair.children[1])) / 64 }));
    return [{ kind: "recipe", path: doc.path, parts: address(root, [index]), label,
      category: "Machine Assembly", form: "Shapeless",
      product: "Block(" + block + ")", count: Number(textOf(field(row, "yield_count"))),
      inputCodes: inputs.map(item => item.code), inputs, patternSlots: null }];
  });
}

export function fabricationRecipes(ctx, doc) {
  const root = ctx.tree(doc);
  const stack = ([resource, units]) => ({ code: "Resource(" + resource + ")", count: units / 64 });
  return fabricationRows(doc.text).map(row => {
    const inputs = row.inputs.map(stack);
    const output = stack(row.output);
    return {
      kind: "recipe", path: doc.path, parts: address(root, [row.id]), label: row.name,
      category: "Fabricator", form: "Powered Process", inputs, inputCodes: inputs.map(item => item.code),
      product: output.code, count: output.count, patternSlots: null,
      description: `${row.ticks / 4} s at ${row.power} kW · ${row.ticks * row.power / 4} kJ per batch`,
    };
  });
}

const locationSources = [
  ["Surface ruins", "crates/worldgen/src/sites/kind.rs", "Ruins"],
  ["Surface sites", "crates/worldgen/src/sites.rs", "Ruins"],
  ["Anchor city", "crates/worldgen/src/anchor.rs", "Cities"],
  ["Undercrown", "crates/worldgen/src/undercrown.rs", "Underground"],
  ["Dungeons", "crates/worldgen/src/dungeon.rs", "Underground"],
  ["Orbital stations", "assets/station_habitat.ron", "Orbital"],
  ["Caves", "crates/worldgen/src/caves.rs", "Underground"],
];
const graphFile = (label, path, category) => ({ kind: "file", label, path, category });

export async function itemRows(ctx, recipes) {
  const [items, materials, foods, machines] = await Promise.all([recipeItems(ctx, recipes),
    ctx.load("assets/materials.ron"), ctx.load("assets/foods.ron"), ctx.load("assets/machines.ron")]);
  const definitions = new Map();
  for (const [doc, fieldName, variant] of [[materials, "id", "Block"], [foods, "kind", "Food"]]) {
    const root = ctx.tree(doc);
    root.children.forEach((row, index) => definitions.set(variant + "(" + textOf(field(row, fieldName)) + ")",
      { path: doc.path, parts: address(root, [index]) }));
  }
  const machineRoot = ctx.tree(machines);
  const machineDefinitions = new Map(machineRoot.children.map((row, index) =>
    ["Block(" + textOf(field(row, "block")) + ")", { path: machines.path, parts: address(machineRoot, [index]) }]));
  return items.map(item => {
    const type = item.code.split("(")[0];
    const category = item.machine ? "Machines" : ({ Block: "Blocks", Food: "Food", Resource: "Materials",
      Tool: "Tools", Weapon: "Weapons", Armor: "Equipment", Cast: "Casts", Bucket: "Containers" }[type] ?? type);
    const source = definitions.get(item.code) ?? { path: type === "Resource"
      ? "crates/app/src/world/expedition.rs" : "crates/app/src/world/inventory.rs" };
    return { ...source, machine: machineDefinitions.get(item.code),
      kind: "item", category, label: item.label, product: item.code };
  });
}

export async function browse(ctx, section) {
  const paths = ctx.paths();
  const existing = new Set(paths);
  if (section === "Creatures" || section === "Animals") {
    const doc = await ctx.load(bestiaryPath);
    const root = ctx.tree(doc);
    return root.children.map((row, index) => {
      const [id, stats] = row.children;
      return { kind: "creature", path: doc.path, parts: address(root, [index]), id: textOf(id),
        body: animalBody(textOf(id), textOf(field(stats, "animal_body")?.children?.[0])),
        label: textOf(field(stats, "name")), category: textOf(field(stats, "role")),
        form: textOf(field(stats, "neutral")) === "true" ? "Neutral" : "",
        detail: textOf(field(stats, "health")) + " health · " + textOf(field(stats, "speed")) + " m/s" };
    }).filter(row => row.label && (section === "Animals" ? isAnimal(row) : !isAnimal(row)));
  }
  if (section === "Biomes") {
    const doc = await ctx.load("assets/biomes.ron");
    const root = ctx.tree(doc);
    const families = field(root, "families");
    return families?.children.map((row, i) => {
      const biome = textOf(field(row, "name"));
      const climate = key => Number(textOf(field(field(row, key), "centre")));
      const temperature = climate("temperature"), moisture = climate("moisture");
      return { kind: "biome", path: doc.path, parts: address(root, [root.children.indexOf(families), i]), biome,
        label: biome.replace(/([a-z])([A-Z])/g, "$1 $2"), color: biomeColors[biome],
        category: temperature < 0.3 ? "Cold" : temperature > 0.6 ? "Warm" : "Temperate",
        detail: moisture < 0.34 ? "Dry" : moisture > 0.64 ? "Wet" : "Moderate moisture" };
    }) ?? [];
  }
  if (section === "Deposits") return deposits.map(deposit => ({ kind: "deposit", path: depositPath,
    deposit: deposit.key, label: deposit.label, category: deposit.form, detail: deposit.description, color: deposit.color }));
  if (section === "Locations") {
    const sites = await ctx.load("crates/worldgen/src/sites/kind.rs");
    const ruins = [...sites.text.matchAll(/Self::\w+(?:\s+if[^=\r\n]+(?:>=|<=)\s*\d+)?\s*=>\s*"([^"\r\n]+)"/g)]
      .map(match => ({ kind: "file", path: sites.path, label: match[1], category: "Ruins",
        line: sites.text.slice(0, match.index).split("\n").length }));
    return [...locationSources.filter(([, path]) => existing.has(path)).map(args => graphFile(...args)),
      ...ruins, ...paths.filter(path => /^assets\/anchor-studio\/designs\/.*\.vxplan$/.test(path))
        .map(path => graphFile(titleCase(filename(path)), path, "Building plans"))];
  }
  if (section === "Models") return paths.filter(path => path.startsWith("assets/") && /\.(vox|glb)$|^assets\/models\/.*\.ron$/i.test(path))
    .map(path => ({ kind: editableVox(path) ? "vox" : /\.glb$/.test(path) ? "glb" : "file",
      path, label: titleCase(filename(path)), category: editableVox(path) ? "Voxel models"
        : /\.glb$/.test(path) ? "GLB meshes" : "Other models", detail: path.split("/").slice(0, -1).join("/") }));
  if (section === "Hand poses") return [
    { kind: "hand", label: "Wand and hand", category: "Wand rig", path: wandPosePath },
    { kind: "hand", label: "Blocks, torches and blades", category: "Item grip", path: itemPosePath },
  ];
  if (section === "Sprites") {
    const sheetCategories = { items16: "Item icons", resources16: "Resource icons", machines16: "Machine icons",
      effects16: "Effect icons", sprites16: "Tool part masks" };
    return (await Promise.all(ctx.spriteFiles().map(async ({ path, sprite }) => {
      if (!isSheet(path)) {
        const spells = path.endsWith("/icons_spells.rs");
        const category = spells ? "Spells & runes" : sprite.mode === "source" ? "Editable textures"
          : sprite.mode === "generated" ? "Generated art" : "Reference art";
        const detail = [sprite.width && sprite.width + " × " + sprite.height,
          sprite.generator ? "Generator" : sprite.mode === "source" ? "Editable" : sprite.mode === "generated"
            ? "Generated" : "Reference", sprite.issue].filter(Boolean).join(" · ");
        return [{ kind: "sprite", path, label: spells ? "Individual spell glyphs" : titleCase(filename(path)), category, sprite, detail }];
      }
      try {
        const doc = await ctx.load(path);
        const sheet = readSheet(doc.text, path);
        const original = ctx.dirty(doc) ? readSheet(doc.original, path) : sheet;
        const category = sheetCategories[filename(path).replace(/\.txt$/, "")];
        const before = new Map(original.entries.map(entry => [entry.id, entry]));
        const look = entry => entry && [entry.rows.map(row => row.text), entry.palette.map(color => color.rgb)].join("|");
        const rampChanged = sheet.masks && sheet.ramps.map(ramp => ramp.palette.map(color => color.rgb)).join("|")
          !== original.ramps.map(ramp => ramp.palette.map(color => color.rgb)).join("|");
        return sheet.entries.map((entry, spriteIndex) => {
          const modified = rampChanged || look(entry) !== look(before.get(entry.id));
          return { kind: "sprite", path, spriteIndex, entry, sprite, modified,
            palette: sheet.masks ? sheet.ramps[0].palette : entry.palette,
            label: titleCase(entry.name), category: entry.name === "spell_frame" ? "Spells & runes" : category,
            detail: "16 × 16 · 1 frame" + (modified ? " · Draft" : "") };
        });
      } catch (error) {
        return [{ kind: "sprite", path, label: titleCase(filename(path)), category: "Unavailable",
          sprite: { mode: "reference", issue: error.message }, detail: error.message }];
      }
    }))).flat();
  }
  return paths.map(path => ({ kind: "file", label: filename(path), path,
    category: path.split("/")[0] === "crates" ? path.split("/").slice(0, 2).join("/") : path.split("/")[0],
    detail: path.slice(0, path.lastIndexOf("/")) }));
}

export async function spawnLinks(ctx, id, biome) {
  const doc = await ctx.load(spawnPath);
  const root = ctx.tree(doc);
  const bestiary = await ctx.load(bestiaryPath);
  const species = new Map(ctx.tree(bestiary).children.map(row => [textOf(row.children[0]), row.children[1]]));
  return root.children.flatMap((row, index) => {
    const [kind, habitat] = row.children;
    const value = textOf(kind);
    const zones = field(habitat, "biomes")?.children.map(textOf) ?? [];
    if (id !== undefined ? value !== id : !zones.includes(biome)) return [];
    const realms = field(habitat, "realms")?.children.map(textOf).join(", ");
    const time = textOf(field(habitat, "time")) || "Any time";
    const stats = species.get(value);
    const name = textOf(field(stats, "name")) || "Creature " + value;
    return [{ kind: "record", path: doc.path, parts: address(root, [index]), species: value, name,
      body: animalBody(value, textOf(field(stats, "animal_body")?.children?.[0])),
      realm: realms, time, weight: textOf(field(habitat, "weight")), habitats: zones.join(", "),
      label: id === undefined ? name : realms || "Spawn rule",
      detail: [id === undefined ? realms : zones.join(", "), time].filter(Boolean).join(" · ") }];
  });
}

