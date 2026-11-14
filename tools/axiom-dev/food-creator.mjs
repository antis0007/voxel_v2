import { el } from "./inspector.mjs";
import { foodPaths, foodTemplates, foodIconId } from "./food-item.mjs";
import { readSheet } from "./sprite-sheet.mjs";
import { hueControl, label, tintPreviews } from "./tint-controls.mjs";

const select = options => el("select", {}, options.map(([value, name]) => el("option", { value }, name)));
const quantity = min => el("input", { type: "number", min, max: 100, step: "any", required: true });
const hint = text => el("p", { className: "muted" }, text);

export async function createFood(ctx, onCreated, preset, sourceSprite) {
  try {
    const docs = await Promise.all(foodPaths.map(path => ctx.load(path)));
    if (docs.some(ctx.dirty)) throw new Error("Save or undo open food, inventory, and item sprite drafts before creating an item.");
    const templates = foodTemplates(docs[1].text);
    const sprites = readSheet(docs[2].text, foodPaths[2]).entries;
    const base = select(templates.map(food => [food.kind, food.name]));
    base.value = preset && templates.some(food => food.kind === preset) ? preset : "Bread";
    const sprite = select(sprites.map(row => [row.id.split(" · ")[0], row.name.replaceAll("_", " ")]));
    const name = el("input", { type: "text", maxLength: 60, required: true, placeholder: "e.g. Ember Bread", autofocus: true });
    const hunger = quantity(0), health = quantity(-100), saturation = quantity(0);
    const effects = el("input", { type: "checkbox" });
    const previews = tintPreviews("New item");
    const hue = hueControl(0, draw);
    const icon = () => sprites[sprite.selectedIndex];
    function draw() { previews.sprite(icon(), Number(hue.input.value)); }
    function chooseBase() {
      const food = templates.find(food => food.kind === base.value);
      for (const [key, input] of Object.entries({ hunger, health, saturation })) input.value = food[key] || "0";
      const id = Number(new RegExp("^\\s*" + food.kind + "\\s*=\\s*(\\d+),", "m").exec(docs[0].text)?.[1]);
      const source = String(foodIconId(food.row, id));
      if (sprites.some(row => row.id.startsWith(source + " · "))) sprite.value = source;
      effects.checked = false;
      draw();
    }
    base.onchange = chooseBase;
    sprite.onchange = draw;
    chooseBase();
    if (sourceSprite !== undefined) {
      sprite.value = String(sourceSprite);
      draw();
    }
    const form = el("div", { className: "idea-fields" }, hint("Create an edible item from an existing food. Add a recipe after creation."),
      label("Item name", name), label("Start from", base),
      el("details", {}, el("summary", {}, "Nutrition and effects"),
        el("div", { className: "item-nutrition" }, label("Hunger restored", hunger), label("Health restored", health),
          label("Fullness buffer", saturation)), label("Copy the base food's status effects", effects)),
      el("details", {}, el("summary", {}, "Artwork"),
        el("div", { className: "item-art" }, label("Placeholder sprite", sprite), hue.view, previews.view)));
    function edit() {
      ctx.dialog("New food item", [form], "Review item", async () => {
        const draft = { name: name.value, template: base.value, sprite: Number(sprite.value), hue: Number(hue.input.value),
          hunger: Number(hunger.value), health: Number(health.value), saturation: Number(saturation.value), copyEffects: effects.checked };
        const review = await ctx.foodWorkflow({ op: "preview", request: { draft } });
        if (docs.some(doc => review.revisions.find(source => source.path === doc.path)?.version !== doc.version)) {
          throw new Error("A food source changed. Reload the item editor before review.");
        }
        const plan = review.item;
        const check = tintPreviews("New item");
        check.sprite(icon(), draft.hue);
        ctx.dialog("Review new item", [
          el("h3", {}, plan.name),
          hint("Food #" + plan.id + " · " + draft.hunger + " hunger · " + draft.health + " health · " + draft.saturation + " fullness"),
          check.view,
          hint("Source preview · Native rebuild and playtest required"),
          el("details", {}, el("summary", {}, "Dependencies · " + review.paths.length + " files"),
            el("ul", {}, review.paths.map(path => el("li", {}, path)))),
          hint("Item IDs remain allocated after creation. Recipe withdrawal preserves existing saves."),
          el("button", { type: "button", onclick: edit }, "Back to editing"),
        ], "Create item", async () => {
          if (docs.some(ctx.dirty)) throw new Error("An open file has a draft. Save or undo it before creating this item.");
          const created = await ctx.foodWorkflow({ op: "apply", request: { draft, approval: review.approval } });
          onCreated("Food(" + created.item.kind + ")");
          ctx.note(created.draftConflicts?.length ? "Saved item; local drafts retained. Reload: " + created.draftConflicts.join(", ") : "Created " + plan.name);
          return true;
        });
        return false;
      });
    }
    edit();
  } catch (error) { ctx.note(error.message, true); }
}
