import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { read, save } from "./workspace.mjs";
import { parse } from "./syntax.mjs";
import { recipeRows } from "./catalog-data.mjs";
import { advancementPath, recipeItems, recipePath } from "./idea-data.mjs";
import { previewRecipeBlocks, readRecipeWiring } from "./recipe-wiring-data.mjs";
import { advancementRecords, recipeBlocks, reparentAdvancement } from "./wiring-commands.mjs";
import { previewSpellBlocks, readSpells, spellFields, spellPath, spellSummary } from "./spell-wiring.mjs";

const load = path => ({ ...read(path), path });
const ctx = { load, parse, tree: doc => parse(doc.text, doc.kind) };
const pick = (rows, key, value) => {
  const matches = rows.filter(row => row[key] === value);
  if (matches.length !== 1) throw new Error("Choose an exact, unique " + key + " from the list command.");
  return matches[0];
};
const recipeSummary = ({ name, shaped, slots, product, count, table, description }) =>
  ({ name, shaped, slots, product, count, table, description });
const advancementSummary = ({ id, parent }) => ({ id, parent: parent || null });

async function run([command = "help", arg, ...rest]) {
  if (rest.length) throw new Error("Unexpected arguments. Run help for usage.");
  switch (command) {
    case "help":
      return {
        commands: ["recipes [search]", "items [search]", "advancements [search]", "recipe <exact name>", "spells [search]",
          "preview < JSON", "apply < JSON"],
        recipeBlocks,
        recipeRequest: { recipe: "Exact recipe name", blocks: [{ op: "output", item: "Catalog item code", count: 1 }] },
        advancementRequest: { child: "Advancement ID", parent: "Parent ID or null to disconnect" },
        spellRequest: { spell: 96, blocks: [{ op: "set", field: "cost", value: 18 }] },
        spellFields,
        apply: "Repeat the preview request with its returned approval value to write the reviewed result.",
        cells: "Use 1-based cells. Shaped recipes use a 3 × 3 grid numbered left to right. Shapeless cells follow list order.",
      };
    case "spells": {
      const query = (arg ?? "").trim().toLowerCase();
      return readSpells(load(spellPath).text).filter(row => row.values.palette !== "Hidden"
        && [row.id, row.name, row.values.palette].some(value => String(value).toLowerCase().includes(query)))
        .map(spellSummary);
    }
    case "recipes":
    case "items":
    case "advancements": {
      const query = (arg ?? "").toLowerCase();
      const rows = command === "advancements" ? advancementRecords(ctx.tree(load(advancementPath))).map(advancementSummary)
        : command === "items" ? await recipeItems(ctx, load(recipePath))
          : recipeRows(ctx, load(recipePath)).map(({ label, form, category }) => ({ name: label, form, category }));
      return rows.filter(row => Object.values(row).some(value => typeof value === "string" && value.toLowerCase().includes(query)));
    }
    case "recipe": {
      const doc = load(recipePath);
      return recipeSummary(readRecipeWiring(ctx, doc, pick(recipeRows(ctx, doc), "label", arg)));
    }
    case "preview":
    case "apply": {
      if (arg !== undefined) throw new Error("Send the command JSON through standard input.");
      return change(command, JSON.parse(readFileSync(0, "utf8").replace(/^\uFEFF/, "")));
    }
    default: throw new Error("Unknown command. Run help for usage.");
  }
}

async function change(command, request) {
  const { recipe, blocks, child, parent, spell } = request;
  let doc, text, before, after;
  if (Number.isSafeInteger(spell) && recipe === undefined && child === undefined && parent === undefined) {
    doc = load(spellPath);
    ({ text, before, after } = previewSpellBlocks(doc.text, spell, blocks));
  } else if (typeof recipe === "string" && spell === undefined && child === undefined && parent === undefined) {
    doc = load(recipePath);
    const target = pick(recipeRows(ctx, doc), "label", recipe);
    const items = new Map((await recipeItems(ctx, doc)).map(item => [item.code, item]));
    const next = previewRecipeBlocks(ctx, doc, target, blocks, items);
    text = next.text;
    before = recipeSummary(readRecipeWiring(ctx, doc, target));
    after = recipeSummary(next.state);
  } else if (typeof child === "string" && (typeof parent === "string" || parent === null)
      && spell === undefined && recipe === undefined && blocks === undefined) {
    doc = load(advancementPath);
    const records = advancementRecords(ctx.tree(doc));
    before = advancementSummary(pick(records, "id", child));
    text = reparentAdvancement(doc.text, records, child, parent ?? "");
    after = advancementSummary(pick(advancementRecords(parse(text, doc.kind)), "id", child));
  } else throw new Error("Choose one recipe stack, spell stack or advancement connection. See help.");
  const changed = doc.text !== text;
  const approval = changed ? createHash("sha256").update(JSON.stringify([doc.path, doc.version, text])).digest("hex") : null;
  if (command === "apply") {
    if (!approval || request.approval !== approval) throw new Error("Preview this change again and supply its approval value.");
    save(doc.path, { version: doc.version, text });
    return { saved: doc.path, after };
  }
  return { path: doc.path, changed, before, after, approval };
}

try {
  process.stdout.write(JSON.stringify(await run(process.argv.slice(2)), null, 2) + "\n");
} catch (error) {
  process.stderr.write(JSON.stringify({ error: error.message }) + "\n");
  process.exitCode = 1;
}
