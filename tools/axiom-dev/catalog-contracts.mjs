// Authoring contracts point at native schemas; they never duplicate game values.
const freeze = value => {
  if (value && typeof value === "object") {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
};

export const catalogContracts = freeze([
  {
    id: "fabrication", path: "assets/fabrication.ron", label: "Fabricator processes",
    schema: "crates/content/src/fabrication.rs", reader: "crates/app/src/world/factory/recipes.rs",
    validator: { crate: "engine_content", example: "check_fabrication" },
    stableIdSections: [null], activation: "rebuild",
  },
  {
    id: "spells", path: "assets/spells.ron", label: "Spells",
    schema: "crates/content/src/spells.rs",
    reader: "crates/app/src/world/adventure/spell_defs.rs",
    validator: { crate: "engine_content", example: "check_spells" },
    stableIdSections: [], activation: "rebuild",
  },
  {
    id: "tool-parts", path: "assets/tools/parts.ron", label: "Tool parts and schemas",
    schema: "crates/tools/src/parts.rs", reader: "crates/tools/src/lib.rs",
    validator: { crate: "engine_tools", example: "check_tool_parts" },
    stableIdSections: ["part_kinds", "schemas"], activation: "rebuild",
  },
  {
    id: "tool-materials", path: "assets/tools/materials.ron", label: "Tool materials",
    schema: "crates/tools/src/materials.rs", reader: "crates/tools/src/lib.rs",
    validator: { crate: "engine_tools", example: "check_tool_materials" },
    stableIdSections: [null], activation: "rebuild",
  },
  {
    id: "reactions", path: "assets/reactions.ron", label: "Material reactions",
    schema: "crates/content/src/reactions.rs", reader: "crates/content/src/reactions.rs",
    validator: { crate: "engine_content", example: "check_reactions" },
    stableIdSections: [], activation: "rebuild",
  },
]);

export const catalogContract = idOrPath => catalogContracts.find(row => row.id === idOrPath || row.path === idOrPath);
export const nativeCatalogs = freeze(Object.fromEntries(catalogContracts.map(row => [row.path, row.validator.example])));
export const validatorBuilds = freeze(catalogContracts.map(row => [row.validator.crate, row.validator.example]));

export function describeCatalog(idOrPath) {
  const contract = catalogContract(idOrPath);
  if (!contract) throw new Error("Unregistered catalog: " + idOrPath + ". Use catalog-cli.mjs list for supported contracts.");
  return contract;
}
