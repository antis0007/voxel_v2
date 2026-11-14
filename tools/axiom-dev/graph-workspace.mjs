import { el, button, findEntry } from "./inspector.mjs";
import { at, title, decode } from "./syntax.mjs";
import { address, displayKey, isNote, isSystemBoard, noteLinks, resolve, rustTunables, selector, stable, wikiLink } from "./targets.mjs";
import { graphCanvas, nodeKind } from "./graph-canvas.mjs";
import { graphInspector } from "./graph-inspector.mjs";
import { isSprite } from "./sprite-sheet.mjs";
import { links } from "./links.mjs";
import { createIdea } from "./idea-creator.mjs";
import { recipePath, spawnPath, advancementPath, field, textOf } from "./idea-data.mjs";
import { balanceRecipe } from "./quick-balance.mjs";
import { showContextMenu } from "./context-menu.mjs";
import { canAssist, maxSources } from "./agent-contract.mjs";
import { showSystemBoard } from "./system-board.mjs";
import { materialPath } from "./material-properties.mjs";
import { codingActions } from "./coding-actions.mjs";

const systems = [
  ["alloying", "Materials and alloying", "Tool materials, molten recipes, casting and assembled equipment",
    ["crates/tools/src", "crates/content/src", "crates/app/src/world"],
    ["docs/axiom-notes/bronze-alloy-workbench.md", "assets/tools/materials.ron", "assets/reactions.ron",
      "assets/tools/parts.ron", "assets/tools/smeltery.ron", "assets/tools/workshop.ron", "assets/materials.ron",
      "crates/app/src/world/factory/recipes.rs", "crates/app/src/world/smeltery.rs",
      "tools/axiom-dev/alloy-cli.mjs", "tools/axiom-dev/alloy-authoring.mjs"]],
  ["world", "World generation", "Terrain, rivers, biomes and planet generation",
    ["crates/worldgen/src", "crates/voxel/src"], ["crates/worldgen/src/basin.rs", "crates/worldgen/src/planet_climate.rs",
      "crates/worldgen/src/ground_exposure.rs", "assets/biomes.ron", "assets/star_systems.ron"]],
  ["game", "Game systems", "Spells, creatures, crafting, machines and progression",
    ["crates/app/src/world", "crates/content/src"], ["assets/spells.ron", "assets/bestiary.ron", "assets/recipes.ron", "assets/machines.ron", "assets/progression.ron"]],
  ["residents", "Residents and settlement life", "Identity, work, movement, animation and the review route",
    ["crates/app/src/world/travel", "crates/app/src/world/farming/jobs", "crates/worldgen/src/civilization"],
    ["assets/fernwake_residents.ron", "crates/app/src/world/adventure/wildlife/resident_model.rs",
      "crates/app/src/world/adventure/wildlife/vehicle.rs", "crates/app/src/world/adventure/settlement_guard.rs",
      "crates/app/src/world/farming/jobs.rs", "crates/app/src/world/farming/planet_freight.rs",
      "crates/app/src/world/travel/civilization/politics.rs",
      "crates/app/src/world/farming/sidecar.rs", "tools/axiom-dev/RESIDENT_WORK.md"]],
  ["sim", "Simulation", "Cells, fluid rules, reactions and orbital systems",
    ["crates/app/src/simulation", "crates/sim/src", "crates/sim_gpu/src", "crates/space/src"], ["assets/reactions.ron", "assets/materials.ron"]],
  ["dungeons", "Dungeons and impossible spaces", "Room plans, illusions, treasure, encounters and gravity",
    ["crates/worldgen/src/dungeon/v2", "crates/worldgen/src/manyfold", "crates/app/src/manyfold"],
    ["crates/app/src/world/mirage.rs", "crates/app/src/world/adventure/encounters.rs",
      "crates/app/src/world/adventure/magic_fx.rs", "assets/spells.ron", "tools/axiom-dev/DUNGEON_REVIEW.md"]],
  ["render", "Rendering", "Lighting, shaders, meshes and atmosphere",
    ["crates/app/src/render", "crates/app/src/mesh", "crates/app/src/shaders"], ["assets/atmosphere.ron", "assets/environment.ron"]],
  ["player", "Player and multiplayer", "Movement, UI, controls and network code",
    ["crates/app/src/player", "crates/app/src/egui_ui", "crates/app/src/multiplayer", "crates/net/src"], ["assets/keybinds.ron"]],
  ["assets", "Assets", "Catalogs, sprite sheets, audio and textures", ["assets"], []],
  ["code", "All project files", "Source, tools and documentation", [""], []],
  ["notes", "Feature notes", "Markdown notes, backlinks and linked settings", [], []],
];
const project = { kind: "project", label: "Axiom", detail: "Development graph" };
const boards = { kind: "boards", label: "System Boards", detail: "Your pinned controls and source files" };
const basename = path => path.split("/").at(-1);
const pretty = text => displayKey(text).replace(/^\w/, char => char.toUpperCase());
const folder = path => ({ kind: "folder", path, label: basename(path) || "Project files", detail: path || "All indexed files" });
const systemNodes = systems.map(([id, label, detail]) => ({ kind: "system", id, label, detail }));
const keyOf = spec => JSON.stringify([spec.kind, spec.path, spec.id, spec.parts ?? spec.trail, spec.name]);
const brief = value => String(value).replace(/\s+/g, " ").slice(0, 76);
const linkParts = spec => spec.kind === "rust" ? ["rust:" + spec.name] : spec.parts ?? [];
const samePin = (link, target) => link.path === target.path
  && JSON.stringify(link.parts) === JSON.stringify(linkParts(target));
const pinLine = (text, target) => [...text.matchAll(/^- \[\[[^\r\n]+\]\][ \t]*(?:\r?\n|$)/gm)]
  .find(match => {
    const links = noteLinks(match[0]);
    return links.length === 1 && samePin(links[0], target);
  });

function fileNode(path, label = basename(path)) {
  const kind = isNote(path) ? "note" : isSprite(path) ? "sprite"
    : /\.(ron|json)$/.test(path) ? "data" : /\.(rs|wgsl|py|mjs|ps1|toml)$/.test(path) ? "code" : "file";
  return { kind, path, label, detail: path.slice(0, path.lastIndexOf("/")) };
}

function searchNode(row) {
  if (row.kind === "file" || row.kind === "note") return fileNode(row.path, row.label);
  return { ...row, label: pretty(row.label), detail: row.context + (row.value ? " · " + brief(row.value) : "") };
}

function linkedNode(link) {
  if (link.parts[0]?.startsWith("rust:")) {
    return { kind: "rust", path: link.path, name: link.parts[0].slice(5), label: link.label };
  }
  return link.parts.length ? { ...link, kind: "field" } : fileNode(link.path, link.label);
}

function codeLinks(path, text, paths) {
  const parts = path.split("/");
  const filename = parts.pop();
  const parent = parts.join("/");
  const nested = /^(lib|main|mod)\.rs$/.test(filename) ? parent : parent + "/" + filename.replace(/\.rs$/, "");
  const result = new Set();
  for (const match of text.matchAll(/^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;/gm)) {
    for (const base of [nested, parent]) {
      const found = [base + "/" + match[1] + ".rs", base + "/" + match[1] + "/mod.rs"].find(file => paths.has(file));
      if (found) { result.add(found); break; }
    }
  }
  for (const match of text.matchAll(/\binclude(?:_str|_bytes)?!\s*\(\s*"([^"\r\n]+)"/g)) {
    const resolved = [...parts];
    for (const segment of match[1].split("/")) {
      if (segment === "..") resolved.pop();
      else if (segment !== "." && segment) resolved.push(segment);
    }
    const link = resolved.join("/");
    if (paths.has(link)) result.add(link);
  }
  return [...result].filter(file => file !== path).map(file => fileNode(file));
}

export function mountGraph(host, ctx) {
  let selected = project;
  let history = [];
  let request = 0;
  let offset = 0;
  let noteIndex;
  let currentDoc;
  const body = el("div", { className: "map-layout" });
  const crumb = el("div", { className: "map-crumb" });
  const find = button("Find anything", () => search(), "Find files, records, settings and notes (Ctrl+K)", "map-find");
  find.append(el("kbd", {}, "Ctrl K"));
  const tools = el("header", { className: "map-toolbar" }, crumb, find,
    button("System Boards", () => go(boards), "Open your custom system boards"),
    button("Create", () => createIdea(ctx, go, selected.path), "Create a recipe, spawn rule, or advancement", "primary"),
    button("New Note", () => newNote(selected.path && selected.kind !== "folder" ? selected : null), "Create a linked note"));
  host.append(tools, body);

  function go(spec) {
    if (keyOf(spec) === keyOf(selected)) return;
    history = [...history, selected].slice(-36);
    selected = spec;
    offset = 0;
    refresh();
  }

  function contextAction(spec, anchor, point) {
    if (!spec.path) return false;
    const actions = [
      ...codingActions(ctx, spec, go),
      { label: "New Linked Note", run: () => newNote(spec) },
      { label: "Link to Note", run: () => pickNote(spec) },
    ];
    if (stable(linkParts(spec))) actions.unshift({ label: "Pin to System Board", run: () => pickBoard(spec) });
    if (spec.path === materialPath && spec.parts?.length) {
      const openMaterial = async duplicate => {
        try {
          const doc = await ctx.load(spec.path);
          const root = ctx.tree(doc);
          const row = resolve(root, spec.parts.slice(0, 1))?.node;
          const id = textOf(field(row, "id"));
          if (id) ctx.editMaterial(id, duplicate);
        } catch (error) { ctx.note(error.message, true); }
      };
      actions.unshift({ label: "Material settings", run: () => openMaterial(false) },
        { label: "Make material variant", run: () => openMaterial(true) });
    }
    if (spec.path === recipePath && spec.parts?.length === 2) {
      if (ctx.wireRecipe) actions.unshift({ label: "Wire Recipe", run: () => ctx.wireRecipe(spec) });
      actions.push({ label: "Balance Recipe", run: () => balanceRecipe(ctx, spec, changed => { selected = changed; refresh(); }) });
    }
    if (spec.path === advancementPath && spec.parts?.length === 1 && ctx.wireAdvancement) {
      actions.unshift({ label: "Wire Advancement", run: () => ctx.wireAdvancement(spec) });
    }
    if (spec.path === "assets/foods.ron" && spec.parts?.length === 1 && ctx.variantFood) {
      actions.unshift({ label: "New Tinted Food Variant", run: () => ctx.variantFood(spec) });
    }
    if (ctx.assist && canAssist(spec.path)) actions.push({ label: "Ask AI", run: () => ctx.assist(spec) });
    showContextMenu(anchor, actions, point);
    return true;
  }

  async function pickNote(spec) {
    const candidates = (await notes()).filter(note => note.path !== spec.path);
    if (!candidates.length) return newNote(spec);
    findEntry("Link to Note", candidates.map(note => ({ key: note.path, label: note.title, detail: note.path })), null,
      path => linkNode(spec, fileNode(path, candidates.find(note => note.path === path).title)));
  }

  async function pickBoard(spec) {
    const candidates = (await notes()).filter(note => note.systemBoard && note.path !== spec.path);
    findEntry("Pin to System Board", [
      { key: "new", label: "New system board", detail: "Create a board with this pin" },
      ...candidates.map(note => ({ key: note.path, label: note.title, detail: note.path })),
    ], null, path => {
      if (path === "new") newNote(spec, true);
      else linkNode(spec, fileNode(path, candidates.find(note => note.path === path).title));
    });
  }

  async function linkNode(spec, target) {
    if (spec.path === target.path) return;
    try {
      const doc = await ctx.load(target.path);
      if (connect(doc, spec) && keyOf(selected) !== keyOf(target)) go(target);
    } catch (error) { ctx.note(error.message, true); }
  }

  function back() {
    if (!history.length) return;
    selected = history.pop();
    offset = 0;
    refresh();
  }

  const notes = () => noteIndex ??= ctx.api("/api/notes").catch(() => []);

  async function refresh(resetNotes = false) {
    if (resetNotes) noteIndex = undefined;
    const ticket = ++request;
    const original = selected;
    const paths = new Set(ctx.paths());
    currentDoc = undefined;
    crumb.replaceChildren(button("←", back, "Previous node"), button("Axiom", () => go(project), "Project graph"),
      original.kind === "project" ? "" : el("span", {}, "›", el("strong", {}, original.label)));
    crumb.firstElementChild.disabled = !history.length;
    let doc, resolved, constantCount = 0, linkedValues = [], boardActions;
    let outgoing = [];
    const incoming = history.length ? [history.at(-1)] : [];
    let view = original;
    try {
      if (original.kind === "project") {
        outgoing = [boards, ...systemNodes];
      } else if (original.kind === "boards") {
        outgoing = (await notes()).filter(note => note.systemBoard).map(note => fileNode(note.path, note.title));
      } else if (original.kind === "system") {
        const [, , , groups, files] = systems.find(row => row[0] === original.id);
        outgoing = original.id === "notes" ? (await notes()).map(note => fileNode(note.path, note.title))
          : [...groups.filter(path => !path || [...paths].some(file => file.startsWith(path + "/"))).map(folder),
            ...files.filter(path => paths.has(path)).map(path => fileNode(path))];
      } else if (original.kind === "folder") {
        const prefix = original.path ? original.path + "/" : "";
        const children = new Map();
        for (const path of paths) {
          if (!path.startsWith(prefix)) continue;
          const remaining = path.slice(prefix.length);
          const first = remaining.split("/")[0];
          if (!children.has(first)) children.set(first, remaining.includes("/") ? folder(prefix + first) : fileNode(path));
        }
        outgoing = [...children.values()].sort((a, b) => Number(b.kind === "folder") - Number(a.kind === "folder")
          || a.label.localeCompare(b.label));
      } else {
        doc = await ctx.load(original.path);
        if (original.kind === "note") {
          const title = /^#\s+(.+)$/m.exec(doc.text)?.[1]?.trim();
          if (title) view = { ...original, label: title };
          outgoing = [...new Map(noteLinks(doc.text).map(link => [keyOf(linkedNode(link)), linkedNode(link)])).values()];
          linkedValues = outgoing.filter(link => link.kind === "rust" || (link.kind === "field" && stable(link.parts)));
          if (isSystemBoard(doc.text)) {
            boardActions = () => openBoard(doc, view.label);
            linkedValues = [];
          }
        } else if (["ron", "json"].includes(doc.kind)) {
          const root = ctx.tree(doc);
          if (original.trail?.length && !original.parts) {
            const linked = at(root, original.trail);
            if (!linked || title(linked, original.trail.at(-1)) !== original.label) throw new Error("This record moved. Find it by name.");
          }
          const parts = original.parts ?? (original.trail && at(root, original.trail) ? address(root, original.trail) : []);
          resolved = resolve(root, parts);
          if (!resolved) throw new Error("This data node moved or has more than one match");
          const { node, trail } = resolved;
          const base = address(root, trail);
          if (node.raw !== undefined) view = { ...original, kind: "field", parts: base, detail: brief(decode(node.raw)) };
          else {
            view = { ...original, kind: base.length ? "record" : "data", parts: base };
            outgoing = node.children.map((child, index) => ({
              kind: child.children ? "record" : "field", path: doc.path,
              label: pretty(title(child, index)), parts: [...base, selector(node, child, index)],
              detail: child.children ? child.children.length + " values" : brief(decode(child.raw)),
            }));
            if (doc.path === spawnPath && !trail.length) {
              const bestiary = await ctx.load("assets/bestiary.ron");
              const names = new Map(ctx.tree(bestiary).children.map(row =>
                [textOf(row.children[0]), textOf(field(row.children[1], "name"))]));
              outgoing = outgoing.map((child, index) => {
                const row = node.children[index];
                const name = names.get(textOf(row.children[0]));
                const realms = field(row.children[1], "realms")?.children.map(textOf).join(", ");
                return name ? { ...child, label: name, detail: realms || child.detail } : child;
              });
            }
            if (trail.length && doc.kind === "ron") {
              const related = await links(ctx.api, [...paths], doc.path, root, node, trail);
              const reference = row => ({ ...fileNode(row.path, row.label), trail: row.at, detail: row.path });
              incoming.push(...related.incoming.slice(0, 4).map(reference));
              outgoing.unshift(...related.outgoing.slice(0, 6).map(reference));
            }
          }
        } else if (/\.rs$/.test(doc.path)) {
          const constants = rustTunables(doc.text);
          constantCount = constants.length;
          if (original.kind === "rust") {
            const match = constants.find(item => item.name === original.name);
            if (match) view = { ...original, detail: match.raw + " · " + match.type };
          }
          outgoing = [
            ...constants.filter(item => item.name !== original.name).map(item => ({
              kind: "rust", path: doc.path, name: item.name, label: pretty(item.name.toLowerCase()),
              detail: item.raw + " · " + item.type,
            })),
            ...codeLinks(doc.path, doc.text, paths),
          ];
        }
        const backlinks = (await notes()).filter(note => note.path !== doc.path && note.links.some(link =>
          link.path === doc.path && (!original.parts?.length || JSON.stringify(link.parts) === JSON.stringify(original.parts))));
        for (const note of backlinks) {
          if (!incoming.some(node => node.path === note.path)) incoming.push(fileNode(note.path, note.title));
        }
      }
      if (ticket !== request) return;
      currentDoc = doc;
      if (offset >= outgoing.length) offset = 0;
      const inspector = graphInspector(ctx, view, doc, resolved, {
        search, newNote, linkedValues, hasTunables: constantCount > 0, refresh,
        newSystem: () => newNote(null, true), openBoard: boardActions,
        undo: () => {
          ctx.undo(doc);
          let removed = false;
          if (view.parts?.length && ["ron", "json"].includes(doc.kind)) {
            try { removed = !resolve(ctx.tree(doc), view.parts); }
            catch { /* Refresh will report invalid draft syntax. */ }
          }
          if (removed && history.length) back();
          else refresh();
        },
        create: () => createIdea(ctx, go, doc?.path),
        previewRecipe: doc?.path === recipePath ? async () => {
          try {
            await ctx.api("/api/start-recipe", undefined, { text: doc.text });
            ctx.note("Opening the recipe sandbox");
            ctx.playtest();
          } catch (error) { ctx.note(error.message, true); }
        } : null,
        connectNote: () => search(false, target => connect(doc, target)),
        openFile: () => ctx.open(doc.path, resolved?.trail, original.kind === "rust"
          ? doc.text.slice(0, rustTunables(doc.text).find(item => item.name === original.name)?.start ?? 0).split("\n").length
          : undefined),
        coding: codingActions(ctx, original, go),
      });
      body.replaceChildren(graphCanvas(view, incoming, outgoing, go, offset, page => { offset = page; refresh(); }, contextAction, linkNode), inspector);
    } catch (error) {
      if (ticket !== request) return;
      currentDoc = doc;
      body.replaceChildren(graphCanvas(original, incoming, outgoing, go, 0, () => {}), el("aside", { className: "map-inspector" },
        el("header", { className: "map-inspector-title" }, el("h2", {}, original.label)),
        el("div", { className: "map-inspector-body" }, el("p", { className: "error" }, error.message),
          original.path ? button("Open file", () => ctx.open(original.path, undefined, 1)) : null)));
    }
  }

  function newNote(spec, systemBoard = false) {
    const label = systemBoard ? "System name" : "Note title";
    const field = el("input", { type: "text", value: spec ? (spec.label + (systemBoard ? " controls" : " notes")).slice(0, 120) : "",
      placeholder: label, autofocus: true, maxLength: 120, "aria-label": label, required: true });
    const purpose = systemBoard ? el("textarea", { rows: 3, maxLength: 500, "aria-label": "System purpose",
      placeholder: "What should this system do? (Optional)" }) : null;
    ctx.dialog(systemBoard ? "New system board" : "New feature note", purpose ? [field, purpose] : [field],
      systemBoard ? "Create board" : "Create note", async () => {
      if (!field.value.trim()) return false;
      const { path } = await ctx.api("/api/notes", undefined, { title: field.value.trim(), systemBoard, purpose: purpose?.value });
      await ctx.scan();
      noteIndex = undefined;
      if (spec?.path) {
        const target = spec.parts && !stable(spec.parts) ? { ...spec, parts: [] } : spec;
        connect(await ctx.load(path), target);
      }
      go(fileNode(path, field.value.trim()));
      return true;
    });
  }

  function connect(doc, spec) {
    const parts = linkParts(spec);
    if (!stable(parts)) return ctx.note("Link the named record or file for this list value", true);
    const label = ["field", "rust"].includes(spec.kind) && spec.context ? spec.context + " · " + spec.label : spec.label;
    const link = wikiLink(spec.path, parts, label);
    if (isSystemBoard(doc.text) ? noteLinks(doc.text).some(link => samePin(link, spec)) : doc.text.includes(link)) {
      return ctx.note("This link is already pinned");
    }
    if (!ctx.edit(doc, doc.text.trimEnd() + "\n\n- " + link + "\n", true)) return false;
    refresh();
    return true;
  }

  function openBoard(doc, name) {
    const pins = [...new Map(noteLinks(doc.text).map(link => [JSON.stringify([link.path, link.parts]), linkedNode(link)])).values()];
    const pin = fields => search(fields, target => {
      if (target.path === doc.path) return ctx.note("Choose a different file to pin", true);
      if (connect(doc, target)) openBoard(doc, name);
    });
    showSystemBoard(ctx, doc, name, pins, {
      pinSetting: () => pin(true), pinFile: () => pin(false),
      open: go,
      canRemove: target => Boolean(pinLine(doc.text, target)),
      remove: target => {
        const match = pinLine(doc.text, target);
        if (match && ctx.edit(doc, doc.text.slice(0, match.index) + doc.text.slice(match.index + match[0].length), true)) {
          refresh();
          openBoard(doc, name);
        }
      },
      assist: () => ctx.assist([fileNode(doc.path, name), ...new Map(pins
        .filter(target => target.path !== doc.path && canAssist(target.path))
        .map(target => [target.path, target])).values()].slice(0, maxSources),
      "Work on " + name + ". Use the system board and pinned sources to identify the implementation and settings involved.\n\nRequested change: "),
    });
  }

  function search(fields = false, picked) {
    const title = fields ? "Find a setting" : "Find in Axiom";
    const input = el("input", { type: "search", placeholder: "Try “Spark damage” or “basin blend”", autofocus: true, "aria-label": title });
    const list = el("div", { className: "finder-results", "aria-label": "Results" });
    const status = el("p", { className: "muted", role: "status" });
    let hits = [], active = 0, ticket = 0, timer;
    const commands = [
      { kind: "command", id: "recipe", path: recipePath, label: "Create recipe", detail: "Shapeless or a drawn pattern" },
      { kind: "command", id: "spawn", path: spawnPath, label: "Create spawn rule", detail: "Creature, realm, time and weight" },
      { kind: "command", id: "advancement", path: advancementPath, label: "Create advancement", detail: "Trigger, target and parent" },
      { kind: "command", id: "system", label: "New system board", detail: "Pin settings, records and files" },
    ];
    const dialog = el("dialog", { className: "finder command-finder", "aria-label": title,
      onclose: () => { clearTimeout(timer); dialog.remove(); } },
    el("header", { className: "section-title" }, el("h2", {}, title), button("Close", () => dialog.close())),
    input, list, status);
    function pick(index) {
      const spec = hits[index];
      if (!spec) return;
      dialog.close();
      if (picked) picked(spec);
      else {
        ctx.activate();
        if (spec.id === "system" && spec.kind === "command") newNote(null, true);
        else if (spec.kind === "command") createIdea(ctx, go, spec.path);
        else go(spec);
      }
    }
    function draw(entries) {
      hits = entries;
      active = 0;
      list.replaceChildren(...entries.map((spec, index) => {
        const row = button("", () => pick(index), spec.path ?? spec.label, "finder-row");
        row.append(el("div", {}, el("strong", {}, spec.label), el("small", {}, spec.detail ?? spec.path ?? "")),
          el("span", { className: "map-kind " + spec.kind }, nodeKind(spec.kind)));
        row.classList.toggle("active", index === 0);
        return row;
      }));
      status.textContent = entries.length ? "" : "No matches";
    }
    function query() {
      const value = input.value.trim();
      const id = ++ticket;
      clearTimeout(timer);
      const actions = fields || picked ? [] : commands.filter(item => value.toLowerCase().split(/\s+/)
        .every(word => (item.label + " " + item.detail).toLowerCase().includes(word)));
      if (!value) {
        draw(fields || picked ? [] : [...actions, boards, ...systemNodes]);
        if (fields) status.textContent = "Search catalog fields and Rust constants";
        return;
      }
      draw(actions);
      if (!actions.length) status.textContent = "Searching…";
      timer = setTimeout(async () => {
        try {
          const rows = await ctx.api("/api/search?q=" + encodeURIComponent(value) + (fields ? "&fields=1" : ""));
          if (id === ticket && dialog.open) draw([...actions, ...rows.map(searchNode)]);
        } catch (error) { if (id === ticket && dialog.open) status.textContent = error.message; }
      }, 130);
    }
    input.oninput = query;
    input.onkeydown = event => {
      if (!hits.length) return;
      if (event.key === "Enter") { event.preventDefault(); pick(active); return; }
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      active = Math.max(0, Math.min(hits.length - 1, active + (event.key === "ArrowDown" ? 1 : -1)));
      [...list.children].forEach((row, index) => row.classList.toggle("active", index === active));
      list.children[active].scrollIntoView({ block: "nearest" });
    };
    document.body.append(dialog);
    dialog.showModal();
    query();
  }

  return { refresh, search, review: () => ctx.review(currentDoc),
    open: spec => {
      ctx.activate();
      go({ ...spec, kind: spec.parts ? "record" : fileNode(spec.path).kind });
    },
  };
}


