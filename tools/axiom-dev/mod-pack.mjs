import { button, el } from "./inspector.mjs";
import { orbitModel, voxelFaces } from "./model-view.mjs";
import { parseVoxel } from "./voxel-model.mjs";
import { fabricationPath, fabricationRows } from "./fabrication-data.mjs";
import { maxPackReference } from "./agent-contract.mjs";

const note = text => el("p", { className: "muted" }, text);
const label = (name, control) => el("label", {}, el("span", {}, name), control);
const option = (value, text) => el("option", { value }, text);

function download(name, text, type) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  el("a", { href: url, download: name }).click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function openModelStudy(ctx, mod, attach) {
  const pick = el("input", { list: "jar-models", maxlength: 400, placeholder: "Blank picks a supported model" });
  const grid = el("select", {}, option("8", "8³ · factory"), option("16", "16³ · concept"));
  const targets = [...ctx.paths()].filter(path => /^assets\/models\/factory\/[\w-]+\.vox$/.test(path));
  const target = el("select", {}, option("", "Download only"), ...targets.map(path => option(path, path.split("/").at(-1))));
  const scene = el("canvas", { width: 620, height: 360, className: "studio-canvas", "aria-label": "Reconstructed voxel prototype" });
  const status = note("Generate a prototype and drag to orbit.");
  let prototype, faces = [], frame;
  const redraw = orbitModel(scene, () => faces, () => frame);
  const save = button("Download VOX", () => download("axiom-prototype.vox", prototype.text, "text/plain"));
  save.disabled = true;
  const play = button("Playtest Model", async () => {
    try {
      if (!prototype || prototype.size !== 8 || !target.value) throw new Error("Generate an 8³ model and select a factory slot");
      await ctx.api("/api/start-model", undefined, {
        asset: target.value.split("/").at(-1).replace(/\.vox$/, ""), text: prototype.text,
      });
      status.textContent = "Playtest launched in a separate world folder.";
    } catch (error) { status.textContent = error.message; }
  });
  const live = button("Apply to running game", async () => {
    try {
      if(!prototype || prototype.size!==8 || !target.value)throw new Error("Generate an 8³ model and select a factory slot");
      const current=await ctx.api("/api/engine",undefined,{op:"inspect",sections:["models"]});
      const result=await ctx.api("/api/engine",undefined,{op:"model",revision:current.revision,
        asset:target.value.split("/").at(-1).replace(/\.vox$/,""),text:prototype.text});
      status.textContent=result.data.voxels + " voxels · Live preview · Undo in Engine workbench";
    }catch(error){status.textContent=error.message;}
  });
  const generate = button("Generate Model", async () => {
    generate.disabled = save.disabled = true;
    prototype = undefined;
    try {
      const result = await ctx.api("/api/mod-model", pick.value.trim(), { digest: mod.digest, size: Number(grid.value) });
      const model = parseVoxel(result.text);
      faces = voxelFaces(model);
      frame = { center: model.size.map(n => n / 2), radius: Math.max(...model.size) * 0.7 };
      prototype = { ...result, size: model.size[0] };
      status.textContent = result.path + " · " + result.boxes + " cuboids · " + result.occupied + " occupied cells";
      attach?.(status.textContent);
      redraw();
      save.disabled = false;
    } catch (error) {
      status.textContent = error.message;
    } finally {
      generate.disabled = false;
    }
  });
  ctx.dialog("Model Study", [
    el("div", { className: "idea-fields" }, label("JAR model path", pick),
      el("datalist", { id: "jar-models" }, mod.models.map(path => option(path, path))),
      label("Voxel grid", grid), generate, label("Factory model slot", target)),
    scene, status, save, play, live, note("The prototype preserves rough geometry. Materials use approximate colors."),
  ], "Stage factory model draft", async () => {
    if (!prototype || !target.value) throw new Error("Generate a prototype and select a factory model slot");
    if (prototype.size !== 8) throw new Error("Factory model slots use the 8³ grid");
    const doc = await ctx.load(target.value);
    if (!ctx.edit(doc, prototype.text)) return false;
    ctx.note("Factory model draft staged");
    return true;
  });
}

function compareRecipes(rows) {
  const exact = new Map(), outputs = new Map();
  for (const row of rows) for (const recipe of row.recipes?.rows ?? []) {
    const key = JSON.stringify([recipe.type, recipe.output, recipe.inputs, recipe.pattern]);
    if (!exact.has(key)) exact.set(key, []);
    exact.get(key).push({ mod: row.name, recipe: recipe.path, count: recipe.count, output: recipe.output });
    if (!outputs.has(recipe.output)) outputs.set(recipe.output, new Set());
    outputs.get(recipe.output).add(key);
  }
  return {
    duplicateOrYieldConflict: [...exact.values()].filter(rows => rows.length > 1)
      .map(rows => {
        const minYield = Math.min(...rows.map(row => row.count));
        return { output: rows[0].output, minYield, maxYield: Math.max(...rows.map(row => row.count)),
          provisionalYield: minYield, routes: rows.slice(0, 16) };
      }),
    alternatives: [...outputs].filter(([, routes]) => routes.size > 1)
      .map(([output, routes]) => ({ output, distinctRoutes: routes.size })),
  };
}

function nativeRouteAudit(text) {
  const rows = fabricationRows(text).map(row => ({
    name: row.name,
    output: row.output[0],
    inputs: Object.fromEntries(row.inputs.map(([id, count]) => [id, count / row.output[1]])),
    seconds: row.ticks * 16 / row.output[1],
    kJ: row.power * row.ticks * 16 / row.output[1],
    kW: row.power,
  }));
  return rows.map(route => ({
    ...route,
    dominatedBy: rows.filter(other => {
      if (route === other || route.output !== other.output) return false;
      const ids = Object.keys(route.inputs);
      if (ids.length !== Object.keys(other.inputs).length || !ids.every(id => id in other.inputs)) return false;
      const costs = [route.seconds, route.kJ, route.kW, ...ids.map(id => route.inputs[id])];
      const alternatives = [other.seconds, other.kJ, other.kW, ...ids.map(id => other.inputs[id])];
      return alternatives.every((cost, i) => cost <= costs[i]) && alternatives.some((cost, i) => cost < costs[i]);
    }).map(other => other.name),
  }));
}

function orderMods(rows) {
  const ids = new Map();
  for (const row of rows) for (const id of row.coverage?.declared.flatMap(mod => mod.modIds) ?? []) {
    if (ids.has(id)) row.warning = "Duplicate mod ID " + id;
    else ids.set(id, row);
  }
  const remaining = new Set(rows), ordered = [];
  for (const row of rows) {
    const deps = new Set(row.coverage?.declared.flatMap(mod => mod.dependencies) ?? []);
    const bundled = new Set(row.coverage?.bundled?.flatMap(jar => jar.declared?.flatMap(mod => mod.modIds) ?? []) ?? []);
    row.missing = [...deps].filter(id => !ids.has(id) && !bundled.has(id));
    row.deps = [...deps].filter(id => ids.has(id) && ids.get(id) !== row);
  }
  while (remaining.size) {
    const next = [...remaining].filter(row => row.deps.every(id => !remaining.has(ids.get(id))));
    if (!next.length) {
      for (const row of remaining) row.warning ||= "Dependency cycle or blocked by a cycle";
      ordered.push(...remaining);
      break;
    }
    for (const row of next) { remaining.delete(row); ordered.push(row); }
  }
  return ordered;
}

export function openPackIntake(ctx, prepare) {
  const files = el("input", { type: "file", accept: ".jar", multiple: true });
  const names = el("textarea", { rows: 6, maxLength: 24000, placeholder: "One mod name or short description per line" });
  const models = el("input", { type: "checkbox", checked: true });
  const auto = el("input", { type: "checkbox" });
  const status = note("Up to 120 entries. JAR analysis stays local.");
  const summary = el("div");
  let running = false, cancelled = false, result;
  const stop = button("Stop Intake", () => { cancelled = true; });
  stop.disabled = true;
  const save = button("Export Pack Report", () => download("axiom-pack-report.json", JSON.stringify(result, null, 2), "application/json"));
  save.disabled = true;
  const scan = button("Scan Pack", async () => {
    if (running) return;
    const jars = [...files.files];
    const brief = [...new Set(names.value.split(/\r?\n/).map(line => line.trim().slice(0, 220)).filter(Boolean))];
    if (!jars.length && !brief.length || jars.length + brief.length > 120) {
      status.textContent = "Provide between 1 and 120 entries.";
      return;
    }
    running = true;
    cancelled = false;
    stop.disabled = false;
    scan.disabled = save.disabled = true;
    summary.replaceChildren();
    const rows = brief.map(name => ({ name, status: "Description only" }));
    const makeModels = models.checked;
    try {
      for (const file of jars) {
        if (cancelled) break;
        status.textContent = "Scanning " + file.name + " · " + (rows.length + 1) + " / " + (jars.length + brief.length);
        const row = { name: file.name, status: "Scanned" };
        try {
          if (!/\.jar$/i.test(file.name) || file.size > 48 * 1024 * 1024) throw new Error("Expected a JAR under 48 MiB");
          Object.assign(row, await ctx.api("/api/mod-evidence", undefined, file));
          if (makeModels && row.models.length) {
            try { row.prototype = await ctx.api("/api/mod-model", undefined, { digest: row.digest, size: 8 }); }
            catch (error) { row.modelIssue = error.message; }
          }
        } catch (error) { row.status = error.message; }
        rows.push(row);
      }
      const ordered = orderMods(rows);
      const balance = compareRecipes(ordered);
      try { balance.fabricator = nativeRouteAudit((await ctx.load(fabricationPath)).text); }
      catch (error) { balance.fabricatorIssue = error.message; }
      result = { version: 1, complete: !cancelled, mods: ordered, balance };
      const count = rows.reduce((n, row) => n + (row.recipes?.rows.length ?? 0), 0);
      status.textContent = `${cancelled ? "Partial scan" : "Intake complete"} · ${rows.length} mods · ${count} comparable recipes · ${rows.filter(row => row.prototype).length} models`;
      summary.append(note(balance.duplicateOrYieldConflict.length + " duplicate or conflicting yield groups · "
        + balance.alternatives.length + " outputs with alternate routes"),
        ...ordered.map(row => el("details", {}, el("summary", {}, row.name + " · " + row.status),
          note([row.warning, row.missing.length && "Unresolved dependencies: " + row.missing.join(", ")].filter(Boolean).join(" · ")),
          ...(row.features ?? []).slice(0, 3).map(feature => note(feature.split("\n")[0])),
          row.prototype ? button("Download Model", () => download(row.name.replace(/\.jar$/i, "") + ".vox", row.prototype.text, "text/plain")) : null)));
      save.disabled = false;
    } finally {
      running = false;
      scan.disabled = false;
      stop.disabled = true;
    }
  });
  ctx.dialog("Modpack Intake", [
    label("Mod JARs", files), label("Names or descriptions", names), label("Generate one rough model per JAR", models),
    label("Run AI synthesis and stage validated drafts on confirmation", auto),
    scan, stop, save, status, summary,
  ], "Prepare Pack Synthesis", async () => {
    if (running || !result?.mods.length) throw new Error("Finish scanning a pack first");
    const perMod = Math.min(1150, Math.floor((maxPackReference - 8000) / result.mods.length));
    const references = result.mods.map(row => [
      row.name, row.coverage?.declared.flatMap(mod => mod.modIds).join(", "),
      row.warning, row.missing.length && "Unresolved deps " + row.missing.join(", "),
      row.recipes?.total && "Recipes " + row.recipes.rows.length + "/" + row.recipes.total,
      row.features?.slice(0, 4).join(" "), row.prototype?.path,
    ].filter(Boolean).join(" · ").slice(0, perMod));
    const comparisons = JSON.stringify({
      conflicts: result.balance.duplicateOrYieldConflict.slice(0, 10)
        .map(({ output, minYield, maxYield, provisionalYield }) => ({ output, minYield, maxYield, provisionalYield })),
      alternatives: result.balance.alternatives.slice(0, 16),
    });
    await prepare(("Recipe comparison (sampled, exact identifiers): " + comparisons
      + "\nNative Fabricator costs per item (resource IDs, seconds, kJ, kW): " + JSON.stringify(result.balance.fabricator ?? []).slice(0, 2000)
      + "\nPack inventory in dependency order:\n" + references.join("\n")).slice(0, maxPackReference), auto.checked);
    return true;
  });
}
