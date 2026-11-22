import { el, button } from "./inspector.mjs";
import { addEntry, patch, quoted } from "./syntax.mjs";
import { resolve } from "./targets.mjs";
import { field, habitatRealms, spawnPath, textOf } from "./idea-data.mjs";

const animalBodies = new Set(["49", "50", "51", "52"]);
export const animalBody = (id, body) => animalBodies.has(body) ? body : animalBodies.has(id) ? id : "";
export const isAnimal = spec => Boolean(spec.body) || ["16", "17", "42"].includes(spec.id);
const times = [["Any", "Any time"], ["Day", "Day"], ["Night", "Night"]];
const human = name => name.replace(/([a-z])([A-Z])/g, "$1 $2");

export async function editHabitat(ctx, species, rule, done, biome) {
  try {
    const [doc, world] = await Promise.all([ctx.load(spawnPath), ctx.load("crates/worldgen/src/landscape.rs")]);
    const original = doc.text;
    const root = ctx.tree(doc);
    const row = rule && resolve(root, rule.parts)?.node;
    if (rule && !row) throw new Error("This habitat moved. Reopen it.");
    const data = row?.children?.[1];
    const variants = /pub enum Biome\s*\{([^}]+)\}/.exec(world.text)?.[1] ?? "";
    const names = [...variants.matchAll(/^\s*(\w+),/gm)].map(match => match[1]);
    if (!names.length) throw new Error("The list of world biomes is unavailable.");
    const existingBiomes = field(data, "biomes")?.children.map(textOf) ?? (biome ? [biome] : []);
    const existingRealms = field(data, "realms")?.children.map(textOf) ?? ["Surface"];
    const animal = isAnimal(species);
    const weight = el("input", { type: "number", min: 0, max: 255, step: 1, required: true,
      value: textOf(field(data, "weight")) || "3" });
    const time = el("select", {}, times.map(([value, name]) => el("option", { value }, name)));
    const existingTime = textOf(field(data, "time")) || "Any";
    time.value = existingTime;
    const all = el("input", { type: "checkbox", checked: row ? !existingBiomes.length : !animal });
    const choices = names.map(name => [name, el("input", { type: "checkbox", checked: existingBiomes.includes(name) })]);
    const realmChoices = habitatRealms.map(([value, title]) => [value, title,
      el("input", { type: "checkbox", checked: existingRealms.includes(value) })]);
    const chips = el("div", { className: "habitat-biomes" }, choices.map(([name, input]) => el("label", {}, input, human(name))));
    all.onchange = () => { chips.hidden = all.checked; };
    chips.hidden = all.checked;
    ctx.dialog((rule ? "Edit habitat" : "Add habitat") + " · " + species.label, [
      el("div", { className: "idea-fields" },
        el("div", { className: "item-nutrition" }, el("label", {}, "Spawn weight", weight),
          animal && !rule ? null : el("label", {}, "Time of day", time)),
        el("p", { className: "muted" }, animal
          ? "Higher weight makes this animal more likely than other grazers in the same habitat."
          : "Higher weight makes this creature more likely among matching spawn rules."),
        animal && !rule ? null : el("div", { className: "habitat-biomes" },
          realmChoices.map(([, title, input]) => el("label", {}, input, title))),
        el("label", { className: "habitat-any" }, all, "Any biome"),
        chips),
    ], "Apply to draft", () => {
      if (doc.text !== original) throw new Error("Spawn rules changed. Reopen the habitat.");
      const selected = choices.filter(([, input]) => input.checked).map(([name]) => name);
      if (!all.checked && !selected.length) throw new Error("Choose one or more biomes, or enable Any biome.");
      const locations = realmChoices.filter(([, , input]) => input.checked).map(([value]) => value);
      if (!locations.length) throw new Error("Choose at least one realm.");
      if (!all.checked && locations.every(realm => realm === "Nether" || realm === "Shell")) {
        throw new Error("Choose Any biome for a Nether or Shell rule.");
      }
      const biomes = all.checked ? [] : selected;
      const realmText = "[" + locations.join(", ") + "]";
      const biomeText = "[" + biomes.map(name => quoted(name, "ron")).join(", ") + "]";
      const same = (a, b) => a.length === b.length && a.every(value => b.includes(value));
      let text = original;
      if (row) {
        const updates = [
          ["weight", String(Number(weight.value)), Number(textOf(field(data, "weight"))) !== Number(weight.value)],
          ["realms", realmText, !same(locations, existingRealms)],
          ["time", time.value, time.value !== existingTime],
          ["biomes", biomeText, !same(biomes, existingBiomes)],
        ];
        for (const [key, raw, changed] of updates) {
          if (!changed) continue;
          const habitat = resolve(ctx.parse(text, "ron"), rule.parts).node.children[1];
          const node = field(habitat, key);
          text = node ? patch(text, node.start, node.end, raw) : addEntry(text, habitat, key + ": " + raw);
        }
      } else {
        const fields = "(weight: " + Number(weight.value) + ", realms: " + realmText +
          (time.value === "Any" ? "" : ", time: " + time.value) + (biomes.length ? ", biomes: " + biomeText : "") + ")";
        text = addEntry(original, root, "(" + species.id + ", " + fields + ")");
      }
      if (!ctx.edit(doc, text)) return false;
      done();
      ctx.note("Spawn draft ready to review");
      return true;
    });
  } catch (error) { ctx.note(error.message, true); }
}

export function habitatCards(ctx, doc, rules, selected, refresh) {
  const body = el("section", { className: "world-card habitat-card" });
  const state = el("div", { className: "world-actions" });
  const changed = ctx.dirty(doc);
  const review = button("Review spawn changes", () => ctx.review(doc), "", "primary");
  const undo = button("Undo spawn change", () => { ctx.undo(doc); refresh(); });
  if (changed) state.append(review, undo);
  const rows = rules.map(rule => {
    const species = selected ?? { id: rule.species, label: rule.name, body: rule.body };
    return el("div", { className: "habitat-row" },
      el("div", {}, el("strong", {}, selected ? rule.habitats || "Any biome" : rule.name),
        el("small", {}, rule.realm + " · " + rule.time + " · Weight " + rule.weight)),
      button("Edit", () => editHabitat(ctx, species, rule, refresh), "Edit " + species.label + " habitat"));
  });
  const cave = selected?.id === "42";
  body.append(el("div", { className: "world-card-head" }, el("h3", {}, selected ? "Habitats" : "Spawns in this biome"),
    selected && !cave ? button("Add habitat", () => editHabitat(ctx, selected, null, refresh)) : null),
  ...rows,
  cave ? el("p", { className: "muted" }, "Cave salamanders spawn on banks inside karst gardens.") : null,
  !cave && !rows.length ? el("p", { className: "muted" }, "No specific habitat rules.") : null, state);
  return body;
}
