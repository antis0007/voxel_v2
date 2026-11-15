import { el, button, panel } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";
import { nativeArtPanel } from "./native-art-panel.mjs";
import { enginePanel } from "./engine-panel.mjs";
import { layoutPanel } from "./layout-editor.mjs";
import { parse, decode } from "./syntax.mjs";
import { organizePanels } from "./panel-organizer.mjs";

const select = values => el("select", {}, values.map(value => el("option", { value }, value)));
const input = (value, attrs = {}) => el("input", { value, ...attrs });
const field = (name, control) => el("label", {}, name, control);
const number = (value, min, max) => input(String(value), { type: "number", min, max, step: 1, required: true });
const coordinate = value => input(value, { required: true, pattern: "~?([+-]?([0-9]+([.][0-9]*)?|[.][0-9]+))?|~" });
const key = text => text.trim().replace(/\s+/g, "_");
const datalist = name => el("datalist", { id: name });
const flex = (...children) => el("div", { className: "chips" }, ...children);

export function mountPlaytest(host, api, note, visible) {
  let game;
  let updating = false;
  let loaded = false;
  let recent = 0;
  const status = el("span", { className: "badge" }, "Clipboard mode");
  const connection = document.getElementById("connection");
  const info = el("p", { className: "muted" });
  const logs = el("div", { className: "log", role: "log", "aria-label": "Build and game output" });
  const command = input("/help", { "aria-label": "Game command", spellcheck: false });
  const run = button("Copy command", () => send(command.value), "Send to the game or copy for chat", "primary");
  const copy = button("Copy", () => clipboard(command.value), "Copy the command for in-game chat");
  const creatureOptions = datalist("creatures");
  const itemOptions = datalist("items");
  const controls = [];

  async function clipboard(line) {
    try {
      await copyText(line);
      note("Command copied. Paste into Axiom chat.");
    } catch (error) { note(error.message, true); }
  }

  async function send(line) {
    const sent = line.trim();
    if (!sent.startsWith("/")) return note("Commands start with /", true);
    command.value = sent;
    if (!game?.ready) return clipboard(sent);
    try { note((await api("/api/command", undefined, { line: sent })).reply); }
    catch (error) { note(error.message, true); }
    await sync();
  }

  async function manage(route) {
    try { display(await api(route, undefined, {})); }
    catch (error) { note(error.message, true); }
  }

  const build = button("Build game", () => manage("/api/build"),
    "Compile current project files offline. Close the running playtest first.");
  const launch = button("Launch playtest", () => manage("/api/start"),
    "Open the panel build with an isolated save directory and live commands.", "primary");
  const existing = button("Open existing build", () => manage("/api/start?existing"),
    "Open release-fast with isolated saves and clipboard commands.");
  const inspectConvoy = button("Inspect convoy", () => send("/convoy-status"),
    "Read the active Planet freight record and count its materialized drivers");
  inspectConvoy.disabled = true;
  const inspectFactions = button("Inspect factions", () => send("/faction-status"),
    "List charter holdings and households on the active world");
  inspectFactions.disabled = true;
  const inspectAmbience = button("Inspect ambience", () => send("/ambience-status"),
    "Read location, mana density, actual regeneration, local plants, water and sparkle strength");
  inspectAmbience.disabled = true;
  const realmOrder = select(["inspect", "found", "pact", "peace", "war", "supply food", "supply timber", "march", "reinforce", "withdraw"]);
  const realmTarget = input("", { placeholder: "Settlement ID", "aria-label": "Settlement ID" });
  const submitRealm = button("Issue realm order", () => send(`/realm ${realmOrder.value}${realmOrder.value === "march" ? " " + realmTarget.value.trim() : ""}`));
  submitRealm.disabled = true;
  const dungeonView = select(["room:labyrinth", "room:pitfall", "room:crypt", "room:library", "complex:0", "complex:1",
    "manyfold:shell-entry", "manyfold:shell-exit", "manyfold:descent"]);
  const spellEffect = select(["spark", "ember", "frost", "shock", "rift", "lumen"]);
  const dungeonControls = [
    button("Open viewpoint", () => send(`/dungeon-view ${dungeonView.value}`), "Move the solo spectator to an existing generated room"),
    button("Inspect crosshair", () => send("/dungeon-survey"), "Read collision, illusions, mimic identity and local up"),
    button("Preview impact", () => send(`/spell-fx ${spellEffect.value}`), "Preview the native effect without spending mana or changing terrain"),
  ];
  dungeonControls.forEach(control => { control.disabled = true; });

  const castSpark = button("Cast Spark", () => send("/forge-spark"), "Cast through the native wand evaluator");
  const sparkGear = button("Add fixture wand", () => send("/forge-spark-gear"));
  const sparkControls = panel("Forge · Spark", flex(sparkGear, castSpark), el("p", { className: "muted" },
    "Open a solo world and add the fixture wand. Equip it from the hotbar and aim into open space. Resume the game to see the projectile fly."));
  sparkControls.hidden = true;
  const recipeControls = panel("Forge · Recipe",
    button("Get recipe ingredients", () => send("/forge-recipe-gear")),
    el("p", { className: "muted" }, "Open a solo world in the preview, get the ingredients, then use the game’s crafting screen."));
  recipeControls.hidden = true;

  function display(next) {
    game = next;
    sparkControls.hidden = !game.sparkReady;
    recipeControls.hidden = !game.recipeReady;
    status.textContent = game.building ? "Building…" : game.ready ? "Connected"
      : game.running ? "Running · Clipboard mode" : "Game closed · Clipboard mode";
    connection.textContent = "Game connected";
    connection.hidden = !game.ready;
    info.textContent = game.saveDirectory;
    info.title = game.timestamp ? "Panel build from " + new Date(game.timestamp).toLocaleString() : "Panel save folder";
    launch.disabled = !game.built || game.running || game.building;
    existing.disabled = !game.fallback || game.running || game.building;
    build.disabled = game.running || game.building;
    inspectConvoy.disabled = !game.ready;
    inspectFactions.disabled = !game.ready;
    inspectAmbience.disabled = !game.ready;
    submitRealm.disabled = !game.ready;
    dungeonControls.forEach(control => { control.disabled = !game.ready; });
    run.textContent = game.ready ? "Send" : "Copy command";
    copy.hidden = !game.ready;
    controls.forEach(control => { control.textContent = game.ready ? control.runLabel : "Copy command"; });
    const last = game.logs.at(-1)?.id ?? 0;
    if (recent !== last) {
      recent = last;
      logs.textContent = game.logs.map(entry => entry.message).join("\n");
      logs.scrollTop = logs.scrollHeight;
    }
  }

  async function sync() {
    if (updating) return;
    updating = true;
    try { display(await api("/api/game")); }
    catch (error) { if (!game) status.textContent = error.message; }
    finally { updating = false; }
  }

  function form(title, fields, label, line, help) {
    const submit = el("button", { type: "submit", title: help }, "Copy command");
    submit.runLabel = label;
    controls.push(submit);
    return panel(title, el("form", { onsubmit: event => { event.preventDefault(); send(line()); } }, fields, submit));
  }

  const mode = select(["creative", "survival", "adventure", "spectator"]);
  const time = select(["day", "noon", "night", "midnight"]);
  const ticks = number(6000, 0, 24000);
  const weather = select(["clear", "rain", "thunder"]);
  const x = coordinate("~");
  const y = coordinate("~10");
  const z = coordinate("~");
  const item = input("Stone", { required: true, list: "items" });
  const count = number(64, 1, 1024);
  const creature = input("Ruin Skitterer", { required: true, list: "creatures" });
  const rule = select(["keepInventory", "doMobSpawning", "doDaylightCycle", "pvp"]);
  const value = select(["true", "false"]);
  const effect = select(["regeneration", "speed", "slowness", "strength", "weakness", "poison", "fire_resistance"]);
  const seconds = number(30, 1, 3600);
  const level = number(1, 1, 5);
  const border = number(256, 16, 1000000);
  const group = (name, ...forms) => el("section", { className: "cheat-group" },
    el("h2", {}, name), el("div", { className: "cheats" }, forms));

  const brief = el("textarea", { rows: 4, maxlength: 6000, required: true,
    placeholder: "Describe the design, constraints, and any playtest evidence." });
  const reviewStatus = el("p", { role: "status" });
  const reviewOutput = el("div", {});
  const refine = el("button", { type: "submit" }, "Run three design passes");
  const review = el("form", { onsubmit: async event => {
    event.preventDefault();
    refine.disabled = true;
    reviewStatus.textContent = "Drafting, critiquing, and revising…";
    reviewOutput.replaceChildren();
    try {
      const { passes } = await api("/api/refine-design", undefined, { brief: brief.value });
      reviewOutput.replaceChildren(...passes.map((pass, i) =>
        el("details", { className: "source-notes", ...(i === 2 ? { open: true } : {}) },
          el("summary", {}, ["Draft", "Critique", "Revised proposal"][i]),
          ...pass.split(/\n+/).filter(Boolean).map(line => el("p", {}, line)))));
      reviewStatus.textContent = "Proposal ready for review.";
    } catch (error) {
      reviewStatus.textContent = error.message;
    } finally {
      refine.disabled = false;
    }
  } }, field("Design brief", brief), refine);

  host.append(
    el("div", { className: "playtest-header" },
      el("h1", {}, "Playtest"), status),
    nativeArtPanel(api),
    panel("Game", flex(launch, build, existing), el("p", {}, "Open a solo world for live commands."),
      el("details", { className: "source-notes" }, el("summary", {}, "Build and save folder"),
        el("p", {}, "Build again after changing embedded files."), info)),
    sparkControls, recipeControls,
    panel("Convoy", inspectConvoy),
    panel("Location ambience", inspectAmbience),
    enginePanel(api, note),
    layoutPanel(api, note),
    panel("Civilization", flex(inspectFactions, realmOrder, realmTarget, submitRealm)),
    panel("Dungeon and magic", flex(dungeonView, dungeonControls[0], dungeonControls[1]), flex(spellEffect, dungeonControls[2])),
    panel("Design refinement", review, reviewStatus, reviewOutput),
    el("div", { className: "playtest-grid" },
      el("div", {}, group("Player",
        form("Mode", field("Game mode", mode), "Set mode", () => "/gamemode " + mode.value,
          "Creative unlocks flight, construction tools, and Give. Spectator has noclip flight."),
        form("Teleport", flex(field("X (m)", x), field("Y (m)", y), field("Z (m)", z)),
          "Teleport", () => "/tp " + [x.value, y.value, z.value].join(" "),
          "Coordinates are in metres. ~ is relative to the player."),
        form("Give item", [field("Item", item), field("Count", count)], "Give item",
          () => "/give " + key(item.value) + " " + count.value, "Creative mode required. Adds as many as fit in the pack."),
        form("Summon", field("Creature", creature), "Summon", () => "/summon " + key(creature.value),
          "Spawns a creature about 3 m ahead, if the spot is valid."),
        form("Status effect", [field("Effect", effect), flex(field("Seconds", seconds), field("Level", level))],
          "Apply effect", () => "/effect give " + [effect.value, seconds.value, level.value].join(" "),
          "Applies a timed effect to the local player.")),
      group("World",
        form("Weather", field("Preset", weather), "Set weather", () => "/weather " + weather.value,
          "Applies the current world's weather preset."),
        form("Time", field("Preset", time), "Set time", () => "/time set " + time.value,
          "Sets the world clock to a named time."),
        form("Time ticks", field("Ticks from sunrise", ticks), "Set ticks", () => "/time set " + ticks.value,
          "24,000 ticks per day, 6,000 at noon."),
        form("World rule", [field("Rule", rule), field("Value", value)], "Set rule",
          () => "/gamerule " + rule.value + " " + value.value, "Changes the selected world rule."),
        form("Border", field("Radius (m)", border), "Set border", () => "/worldborder " + border.value,
          "Sets a session-only border around the origin."))),
      el("div", {},
        panel("Console", el("form", { className: "command", onsubmit: event => {
          event.preventDefault(); send(command.value);
        } }, command, run, copy),
        flex(...[["Help", "/help"], ["Seed", "/seed"], ["Save world", "/save-all"],
          ["Clear effects", "/effect clear"], ["Border off", "/worldborder off"]].map(([label, line]) =>
          button(label, () => send(line), line))),
        el("p", {}, "Clipboard commands go into Axiom chat.")),
        el("details", { className: "panel output-panel" }, el("summary", {}, "Build and game output"), logs))),
    creatureOptions, itemOptions);

  async function loadOptions() {
    if (loaded) return;
    loaded = true;
    for (const [catalog, target] of [["bestiary", creatureOptions], ["materials", itemOptions]]) {
      try {
        const doc = await api("/api/read", "assets/" + catalog + ".ron");
        const rows = parse(doc.text, "ron").children;
        const names = rows.map(row => (catalog === "bestiary" ? row.children[1] : row)?.children
          ?.find(node => node.key === (catalog === "bestiary" ? "name" : "display_name"))?.raw)
          .filter(Boolean).map(decode);
        target.replaceChildren(...[...new Set(names)].map(name => el("option", { value: name })));
      } catch { /* The text controls remain available when a catalog is invalid. */ }
    }
  }

  const sections = organizePanels(host, {
    key: "axiom-dev.playtest.sections",
    sections: ":scope > .panel, :scope > details.panel, :scope > .playtest-grid .cheat-group,"
      + " :scope > .playtest-grid > div > .panel, :scope > .playtest-grid > div > details",
    categories: [
      ["Session", /^(Game|Console|Build and game output)/],
      ["Forge", /^Forge/],
      ["Commands", /^(Player|World)$/],
      ["Inspect", /Convoy|ambience|Civilization|Engine|Native art|Dungeon/i],
      ["Design", /Design/],
    ],
    before: host.children[1],
  });

  sync();
  setInterval(() => { if (visible()) { sync(); loadOptions(); } }, 1000);
  return { sections };
}

