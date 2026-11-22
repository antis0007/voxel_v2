// Whole-game node graph: catalogs, records and Rust source in one canvas.
// Map view shows every cluster. Trace view shows one node's chain in columns.
// Lenses recolour the map, impact marks everything that depends on a node,
// path finds the chain between two nodes, and pins collect context for an agent.
import { el, button } from "./inspector.mjs";
import { adjacency, impact, shortestPath, linkLabel, findNodes, brief } from "./graph-query.mjs";

const palette = {
  World: "#6fb1ff", Materials: "#ffb454", Magic: "#b79bff", Creatures: "#ff7d8e", Industry: "#f5d76e",
  Crafting: "#5ee6a8", Settlement: "#ef9fd8", Player: "#5fd7ee", Settings: "#8f9db2",
  "Code · app": "#c9d3e0", "Code · worldgen": "#a9c09a", "Code · engine": "#c4b29c",
};
const tint = group => palette[group] ?? "#8f9db2";
const cyan = "#5ee6d0", amber = "#ffb454", red = "#ff5d6c";
const golden = 2.399963;
const boxW = 224, boxH = 42, colGap = 320, rowGap = 50, columnCap = 16;
const issueNames = { dangling: "Broken link", duplicate: "Duplicate id", unused: "Unused", unread: "Unread file", unsynced: "Not in multiplayer" };
const netTone = { replicated: "#5ee6d0", handled: "#ffb454", local: "#ff5d6c" };

function pack(items, gap) {
  // Greedy spiral packing: largest first, each as close to the centre as fits.
  const placed = [];
  for (const item of [...items].sort((a, b) => b.r - a.r)) {
    let spot;
    const unit = Math.max(4, item.r * .3);
    for (let step = 0; !spot && step < 6000; step++) {
      const radius = step ? unit * Math.sqrt(step) : 0;
      const x = Math.cos(step * golden) * radius, y = Math.sin(step * golden) * radius * .78;
      if (placed.every(other => Math.hypot(other.x - x, other.y - y) >= other.r + item.r + gap)) spot = { x, y };
    }
    item.x = spot?.x ?? 0;
    item.y = spot?.y ?? 0;
    placed.push(item);
  }
  if (!placed.length) return 1;
  const midX = (Math.min(...placed.map(item => item.x - item.r)) + Math.max(...placed.map(item => item.x + item.r))) / 2;
  const midY = (Math.min(...placed.map(item => item.y - item.r)) + Math.max(...placed.map(item => item.y + item.r))) / 2;
  for (const item of placed) { item.x -= midX; item.y -= midY; }
  return Math.max(1, ...placed.map(item => Math.hypot(item.x, item.y) + item.r));
}

function layout(graph) {
  const catalogs = graph.catalogs.map((catalog, index) => ({ ...catalog, index, members: [] }));
  graph.nodes.forEach((node, index) => {
    if (node.hub) catalogs[node.c].members.unshift(index); else catalogs[node.c].members.push(index);
  });
  for (const catalog of catalogs) catalog.r = 13 + 8.6 * Math.sqrt(Math.max(0, catalog.members.length - 1));
  const groups = [...new Set(catalogs.map(catalog => catalog.group))].map(name => {
    const members = catalogs.filter(catalog => catalog.group === name);
    return { name, members, code: name.startsWith("Code"), r: pack(members, 9) + 26,
      nodes: members.reduce((sum, catalog) => sum + catalog.count, 0) };
  });
  // Data areas and code areas form two blocks side by side.
  const blocks = [groups.filter(group => !group.code), groups.filter(group => group.code)];
  const reach = blocks.map(block => pack(block, 34));
  blocks[0].forEach(group => { group.x -= reach[0] + 40; });
  blocks[1].forEach(group => { group.x += reach[1] + 40; });
  for (const group of groups) for (const catalog of group.members) { catalog.x += group.x; catalog.y += group.y; }
  const points = new Float32Array(graph.nodes.length * 2);
  for (const catalog of catalogs) {
    const count = catalog.members.length;
    catalog.spacing = (catalog.r - 9) * 1.77 / Math.sqrt(Math.max(1, count));
    catalog.members.forEach((node, order) => {
      const radius = count === 1 ? 0 : Math.sqrt(order / count) * (catalog.r - 9);
      points[node * 2] = catalog.x + Math.cos(order * golden) * radius;
      points[node * 2 + 1] = catalog.y + Math.sin(order * golden) * radius;
    });
  }
  return { catalogs, groups, points };
}

export function gameMap(graph, hooks) {
  const { catalogs, groups, points } = layout(graph);
  const { out, inn } = adjacency(graph);
  const adj = { out, inn };
  const degree = index => out[index].length + inn[index].length;
  const bundles = new Map();
  for (const [from, to] of graph.edges) {
    const a = graph.nodes[from].c, b = graph.nodes[to].c;
    if (a !== b) bundles.set(a + ">" + b, (bundles.get(a + ">" + b) ?? 0) + 1);
  }
  const bundleRows = [...bundles].map(([key, count]) => [...key.split(">").map(Number), count]);
  const catalogLinks = index => {
    const order = (x, y) => y[1] - x[1];
    return { uses: bundleRows.filter(row => row[0] === index).map(row => [row[1], row[2]]).sort(order),
      usedBy: bundleRows.filter(row => row[1] === index).map(row => [row[0], row[2]]).sort(order) };
  };
  const nodeIssues = new Map(), catalogIssues = new Map();
  for (const issue of graph.issues ?? []) {
    const table = issue.node !== undefined ? nodeIssues : catalogIssues, key = issue.node ?? issue.catalog;
    if (!table.has(key)) table.set(key, []);
    table.get(key).push(issue);
  }
  const maxPins = hooks.maxPins ?? 5;

  let selected = null;       // { node } | { catalog } | null
  let mode = "map", depth = 2, trace = null, hover = null, lens = "area";
  let mark = null;           // { title, nodes: Map(node → depth) } set by impact
  let pathFrom = null, pins = [], past = [], future = [];
  const hiddenGroups = new Set();
  let camera = { x: 0, y: 0, s: 1 }, flight, drag, moved = false, fitted = false;

  const canvas = el("canvas", { className: "game-canvas", tabIndex: 0, "aria-label": "Whole game graph" });
  const context = canvas.getContext("2d");
  const tip = el("div", { className: "game-tip", hidden: true });
  const hint = el("div", { className: "game-hint", hidden: true, role: "status" });
  const hud = el("div", { className: "game-hud" });
  const depthLabel = el("span", {});
  const depthBox = el("div", { className: "game-bar", hidden: true },
    button("−", () => setDepth(depth - 1), "Fewer steps ( [ )"), depthLabel, button("+", () => setDepth(depth + 1), "More steps ( ] )"));
  const finder = el("input", { type: "search", className: "game-find", placeholder: "FIND RECORD, CATALOG OR SOURCE FILE   /", "aria-label": "Find in the game graph" });
  const finderList = el("div", { className: "game-find-results", hidden: true });
  const lensBar = el("div", { className: "game-bar" });
  const modeBar = el("div", { className: "game-bar" });
  const layerBar = el("div", { className: "game-layers" });
  const dock = el("div", { className: "game-dock", hidden: true });
  const side = el("aside", { className: "map-inspector game-side" });
  const stage = el("div", { className: "map-viewport game-stage" }, canvas, tip,
    el("div", { className: "game-top" }, el("div", { className: "game-find-box" }, finder, finderList), lensBar, layerBar),
    hint, dock,
    el("div", { className: "game-bottom" },
      el("div", { className: "game-bar" }, button("−", () => zoomBy(1 / 1.3), "Zoom out"), button("+", () => zoomBy(1.3), "Zoom in"),
        button("FIT", () => fit(true), "Show everything (0)")),
      modeBar, depthBox, hud));
  const element = el("div", { className: "map-layout game-layout" }, stage, side);

  const kindOf = index => graph.nodes[index].file ? "code" : graph.nodes[index].hub ? "catalog" : "record";
  const catalogSpec = catalog => catalog.kind === "code"
    ? { kind: "folder", path: catalog.path, label: catalog.label, detail: catalog.path }
    : { kind: "data", path: catalog.path, label: catalog.path.split("/").at(-1), detail: "assets" };
  const nodeSpec = index => {
    const node = graph.nodes[index], catalog = catalogs[node.c];
    if (node.file) return { kind: "code", path: node.file, label: node.file.split("/").at(-1), detail: node.file };
    if (node.hub) return catalogSpec(catalog);
    return { kind: "record", path: catalog.path, label: node.title, trail: node.at, detail: catalog.path };
  };
  const specOf = choice => choice?.node !== undefined ? nodeSpec(choice.node) : choice?.catalog !== undefined ? catalogSpec(catalogs[choice.catalog]) : null;
  const colour = index => tint(catalogs[graph.nodes[index].c].group);
  const shown = catalog => !hiddenGroups.has(catalog.group);
  const nodeShown = index => shown(catalogs[graph.nodes[index].c]);
  const number = value => value.toLocaleString("en-US");

  // ---- trace layouts -----------------------------------------------------
  function buildTrace(start) {
    const columns = new Map([[0, [start]]]);
    const column = new Map([[start, 0]]);
    const hidden = new Map();
    for (const [direction, lists] of [[1, out], [-1, inn]]) {
      let frontier = [start];
      for (let step = 1; step <= depth && frontier.length; step++) {
        const next = [];
        for (const from of frontier) for (const [to] of lists[from]) {
          if (!column.has(to)) { column.set(to, direction * step); next.push(to); }
        }
        next.sort((a, b) => Number(Boolean(graph.nodes[a].file)) - Number(Boolean(graph.nodes[b].file))
          || graph.nodes[a].c - graph.nodes[b].c || degree(b) - degree(a));
        const kept = next.slice(0, columnCap);
        for (const dropped of next.slice(columnCap)) column.delete(dropped);
        if (next.length > kept.length) hidden.set(direction * step, next.length - kept.length);
        if (kept.length) columns.set(direction * step, kept);
        frontier = kept;
      }
    }
    const place = new Map();
    for (const [col, list] of columns) {
      list.forEach((node, row) => place.set(node, { x: col * colGap, y: (row - (list.length - 1) / 2) * rowGap, col }));
    }
    const wires = [];
    for (const [from, at] of place) for (const [to, label] of out[from]) {
      const target = place.get(to);
      if (target && target.col !== at.col) wires.push({ from, to, label });
    }
    const extras = [...hidden].map(([col, count]) => ({ x: col * colGap,
      y: ((columns.get(col)?.length ?? 0) + 1) / 2 * rowGap, text: "+" + count + " MORE · SEE PANEL" }));
    return { place, wires, extras, heads: [[-1, "◀ USED BY"], [0, "SELECTED"], [1, "USES ▶"]] };
  }
  function buildChain(chain) {
    const place = new Map(chain.map((node, order) => [node, { x: order * colGap, y: 0, col: order }]));
    const wires = [];
    for (let order = 1; order < chain.length; order++) {
      const link = linkLabel(adj, chain[order - 1], chain[order]);
      wires.push(link.forward ? { from: chain[order - 1], to: chain[order], label: link.label + " ▶" }
        : { from: chain[order], to: chain[order - 1], label: "◀ " + link.label, back: true });
    }
    return { place, wires, extras: [], heads: [[0, "FROM"], [chain.length - 1, "TO"]], chain };
  }

  // ---- camera ------------------------------------------------------------
  const view = () => ({ w: canvas.clientWidth, h: canvas.clientHeight });
  const toScreen = (x, y) => [(x - camera.x) * camera.s + view().w / 2, (y - camera.y) * camera.s + view().h / 2];
  const toWorld = (x, y) => [(x - view().w / 2) / camera.s + camera.x, (y - view().h / 2) / camera.s + camera.y];
  function fly(x, y, s, instant) {
    s = Math.max(.05, Math.min(9, s));
    if (instant) { camera = { x, y, s }; flight = undefined; return draw(); }
    const from = { ...camera }, began = performance.now();
    const step = () => {
      if (flight !== step) return;
      const t = Math.min(1, (performance.now() - began) / 240), ease = t * (2 - t);
      camera = { x: from.x + (x - from.x) * ease, y: from.y + (y - from.y) * ease,
        s: Math.exp(Math.log(from.s) + (Math.log(s) - Math.log(from.s)) * ease) };
      draw();
      if (t < 1) requestAnimationFrame(step); else flight = undefined;
    };
    flight = step;
    requestAnimationFrame(step);
  }
  function frame(minX, minY, maxX, maxY, instant, limit = 9) {
    const { w, h } = view();
    if (!w || !h) return;
    fly((minX + maxX) / 2, (minY + maxY) / 2, Math.min(limit, (w - 70) / Math.max(1, maxX - minX), (h - 150) / Math.max(1, maxY - minY)), instant);
  }
  function frameNodes(list, limit = 5) {
    if (!list.length) return;
    const xs = list.map(node => points[node * 2]), ys = list.map(node => points[node * 2 + 1]);
    frame(Math.min(...xs) - 40, Math.min(...ys) - 40, Math.max(...xs) + 40, Math.max(...ys) + 40, false, limit);
  }
  function fit(animated) {
    if (mode === "trace" && trace) {
      const spots = [...trace.place.values()];
      return frame(Math.min(...spots.map(p => p.x)) - boxW / 2, Math.min(...spots.map(p => p.y)) - boxH * 1.6,
        Math.max(...spots.map(p => p.x)) + boxW / 2, Math.max(...spots.map(p => p.y)) + boxH * 1.6, !animated, 1.4);
    }
    const visible = groups.filter(group => !hiddenGroups.has(group.name));
    if (!visible.length) return;
    frame(Math.min(...visible.map(g => g.x - g.r)), Math.min(...visible.map(g => g.y - g.r)),
      Math.max(...visible.map(g => g.x + g.r)), Math.max(...visible.map(g => g.y + g.r)), !animated);
  }
  function zoomBy(factor, sx = view().w / 2, sy = view().h / 2) {
    const [wx, wy] = toWorld(sx, sy);
    const s = Math.max(.05, Math.min(9, camera.s * factor));
    flight = undefined;
    camera = { x: wx - (sx - view().w / 2) / s, y: wy - (sy - view().h / 2) / s, s };
    draw();
  }

  // ---- drawing -----------------------------------------------------------
  function curve(ax, ay, bx, by, bend = .18) {
    context.beginPath();
    context.moveTo(ax, ay);
    context.quadraticCurveTo((ax + bx) / 2 - (by - ay) * bend, (ay + by) / 2 + (bx - ax) * bend, bx, by);
    context.stroke();
  }
  function text(value, x, y, size, fill, align = "center", weight = 500, mono = false) {
    context.font = weight + " " + size + "px " + (mono ? "ui-monospace, 'Cascadia Mono', Consolas, monospace" : "system-ui, 'Segoe UI', sans-serif");
    context.textAlign = align;
    context.lineWidth = 3;
    context.strokeStyle = "#080c13";
    context.strokeText(value, x, y);
    context.fillStyle = fill;
    context.fillText(value, x, y);
  }
  const clip = (value, max) => value.length > max ? value.slice(0, max - 1) + "…" : value;
  function dotStyle(node, base) {
    if (lens === "issues") return nodeIssues.has(node) ? [red, 1, 1.9] : ["#5a6678", .35, .8];
    if (lens === "heat") {
      const heat = Math.min(1, Math.log2(degree(node) + 1) / 6);
      return [heat > .66 ? "#ffffff" : heat > .33 ? amber : "#56708f", .3 + heat * .7, .7 + heat * 1.5];
    }
    if (lens === "net") {
      const state = graph.nodes[node].net;
      return state ? [netTone[state], 1, state === "local" ? 1.9 : 1.3] : ["#3d4858", .3, .75];
    }
    if (lens === "kind") return [graph.nodes[node].file ? "#c9d3e0" : graph.nodes[node].hub ? "#ffffff" : cyan, 1, graph.nodes[node].hub ? 1.6 : 1];
    return [base, degree(node) ? 1 : .45, graph.nodes[node].hub ? 1.7 : 1];
  }

  function drawMap() {
    const { w, h } = view(), s = camera.s;
    const focusNode = selected?.node, focusCatalog = selected?.catalog;
    const lit = new Set();
    if (mark) for (const node of mark.nodes.keys()) lit.add(graph.nodes[node].c);
    if (focusNode !== undefined) {
      lit.add(graph.nodes[focusNode].c);
      if (!mark) for (const [other] of [...out[focusNode], ...inn[focusNode]]) lit.add(graph.nodes[other].c);
    } else if (focusCatalog !== undefined) {
      lit.add(focusCatalog);
      const related = catalogLinks(focusCatalog);
      for (const [other] of [...related.uses, ...related.usedBy]) lit.add(other);
    }
    const dimmed = index => lit.size && !lit.has(index);
    for (const group of groups) {
      if (hiddenGroups.has(group.name)) continue;
      const [x, y] = toScreen(group.x, group.y);
      context.globalAlpha = 1;
      context.fillStyle = tint(group.name) + "0b";
      context.strokeStyle = tint(group.name) + "38";
      context.lineWidth = 1;
      context.setLineDash([2, 5]);
      context.beginPath();
      context.arc(x, y, group.r * s, 0, 7);
      context.fill();
      context.stroke();
      context.setLineDash([]);
    }
    // Catalog-to-catalog bundles.
    const hot = hover?.catalog ?? focusCatalog;
    if (focusNode === undefined && !mark) {
      for (const [a, b, count] of bundleRows) {
        if (!shown(catalogs[a]) || !shown(catalogs[b])) continue;
        const touches = hot === a || hot === b;
        if (hot !== undefined ? !touches : count < (catalogs[a].kind === "code" || catalogs[b].kind === "code" ? 24 : 1)) continue;
        const [ax, ay] = toScreen(catalogs[a].x, catalogs[a].y), [bx, by] = toScreen(catalogs[b].x, catalogs[b].y);
        context.globalAlpha = touches ? .85 : Math.min(.4, .05 + count / 260);
        context.strokeStyle = touches ? (hot === a ? cyan : amber) : "#5b7ba8";
        context.lineWidth = Math.min(7, .8 + Math.log2(count + 1) * (touches ? 1 : .7));
        curve(ax, ay, bx, by);
      }
    }
    const dots = [];
    for (const catalog of catalogs) {
      if (!shown(catalog)) continue;
      const [x, y] = toScreen(catalog.x, catalog.y), r = catalog.r * s;
      if (x + r < 0 || y + r < 0 || x - r > w || y - r > h) continue;
      const base = tint(catalog.group);
      const flagged = lens === "issues" && catalogIssues.has(catalog.index);
      context.globalAlpha = dimmed(catalog.index) ? .22 : 1;
      context.fillStyle = "#101826cc";
      context.strokeStyle = focusCatalog === catalog.index ? "#ffffff" : flagged ? red : base + (hover?.catalog === catalog.index ? "ff" : "77");
      context.lineWidth = focusCatalog === catalog.index ? 2.2 : 1;
      context.beginPath();
      context.arc(x, y, r, 0, 7);
      context.fill();
      context.stroke();
      const spacing = catalog.spacing * s;
      if (spacing > 3) {
        const size = Math.max(1.2, Math.min(5.5, spacing * .2));
        for (const node of catalog.members) {
          const px = (points[node * 2] - camera.x) * s + w / 2, py = (points[node * 2 + 1] - camera.y) * s + h / 2;
          if (px < -9 || py < -9 || px > w + 9 || py > h + 9) continue;
          const [fill, alpha, scale] = dotStyle(node, base);
          context.globalAlpha = dimmed(catalog.index) ? .2 : mark && !mark.nodes.has(node) && node !== focusNode ? .25 : alpha;
          context.fillStyle = fill;
          context.beginPath();
          if (graph.nodes[node].file) context.rect(px - size * scale, py - size * scale, size * scale * 2, size * scale * 2);
          else context.arc(px, py, size * scale, 0, 7);
          context.fill();
          if (lens !== "issues" && nodeIssues.has(node) && spacing > 9) {
            context.strokeStyle = red;
            context.lineWidth = 1.2;
            context.beginPath();
            context.arc(px, py, size * scale + 2.5, 0, 7);
            context.stroke();
          }
          if (spacing > 64) dots.push([node, px, py + size * scale + 12]);
        }
      } else {
        context.globalAlpha = dimmed(catalog.index) ? .1 : .28;
        context.fillStyle = flagged ? red : base;
        context.beginPath();
        context.arc(x, y, Math.max(1, r - 3), 0, 7);
        context.fill();
      }
    }
    context.globalAlpha = 1;
    for (const [node, px, py] of dots) text(clip(graph.nodes[node].label, 22), px, py, 11, "#cfdbea");
    for (const catalog of catalogs) {
      if (!shown(catalog)) continue;
      const [x, y] = toScreen(catalog.x, catalog.y), r = catalog.r * s;
      if (r < 16 && hover?.catalog !== catalog.index && !(lit.has(catalog.index) && r > 5)) continue;
      if (x < -80 || y < -40 || x > w + 80 || y > h + 40) continue;
      context.globalAlpha = dimmed(catalog.index) ? .35 : 1;
      const above = r > 58;
      text(catalog.label.toUpperCase(), x, above ? y - r - 6 : y + 4, Math.max(9.5, Math.min(13, r * .26)), "#eef4fb", "center", 650, true);
      if (!above && r > 28 && catalog.count) text(String(catalog.count), x, y + 18, 9.5, tint(catalog.group), "center", 500, true);
    }
    context.globalAlpha = 1;
    for (const group of groups) {
      if (hiddenGroups.has(group.name)) continue;
      const [x, y] = toScreen(group.x, group.y - group.r);
      if (group.r * s > 40) text("// " + group.name.toUpperCase(), x, y - 8, Math.max(11, Math.min(19, group.r * s * .085)), tint(group.name), "center", 700, true);
    }
    const spot = node => [(points[node * 2] - camera.x) * s + w / 2, (points[node * 2 + 1] - camera.y) * s + h / 2];
    if (mark) {
      const labelled = mark.nodes.size <= 60 || s > 2.4;
      for (const [node, steps] of mark.nodes) {
        if (!nodeShown(node)) continue;
        const [px, py] = spot(node);
        context.globalAlpha = Math.max(.35, 1 - steps * .16);
        context.strokeStyle = steps === 1 ? amber : "#ff8a5c";
        context.lineWidth = 1.6;
        context.beginPath();
        context.arc(px, py, 5.5, 0, 7);
        context.stroke();
        if (labelled) text(clip(graph.nodes[node].label, 22), px, py - 9, 11, "#ffe2bd");
      }
      context.globalAlpha = 1;
    }
    if (focusNode !== undefined && nodeShown(focusNode)) {
      const [px, py] = spot(focusNode);
      if (!mark) {
        const labelled = degree(focusNode) <= 70 || s > 2.4;
        for (const [list, stroke] of [[out[focusNode], cyan], [inn[focusNode], amber]]) {
          for (const [other] of list) {
            if (!nodeShown(other)) continue;
            const [ox, oy] = spot(other);
            context.globalAlpha = .75;
            context.strokeStyle = stroke;
            context.lineWidth = 1.2;
            curve(px, py, ox, oy, .12);
            context.globalAlpha = 1;
            context.fillStyle = stroke;
            context.beginPath();
            context.arc(ox, oy, 4.2, 0, 7);
            context.fill();
            if (labelled) text(clip(graph.nodes[other].label, 22), ox, oy - 8, 11, "#eef4fb");
          }
        }
      }
      context.strokeStyle = "#ffffff";
      context.lineWidth = 1.5;
      const reach = 11;
      for (const [dx, dy] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
        context.beginPath();
        context.moveTo(px + dx * reach, py + dy * (reach - 5));
        context.lineTo(px + dx * reach, py + dy * reach);
        context.lineTo(px + dx * (reach - 5), py + dy * reach);
        context.stroke();
      }
      context.fillStyle = "#ffffff";
      context.beginPath();
      context.arc(px, py, 5, 0, 7);
      context.fill();
      text(graph.nodes[focusNode].label, px, py - 17, 14, "#ffffff", "center", 700);
    }
    for (const node of pins) {
      if (!nodeShown(node)) continue;
      const [px, py] = spot(node);
      context.strokeStyle = cyan;
      context.lineWidth = 1.6;
      context.strokeRect(px - 7, py - 7, 14, 14);
    }
  }

  function drawTrace() {
    const s = camera.s;
    for (const wire of trace.wires) {
      const a = trace.place.get(wire.from), b = trace.place.get(wire.to);
      const forward = b.x > a.x;
      const [ax, ay] = toScreen(a.x + (forward ? boxW / 2 : -boxW / 2), a.y);
      const [bx, by] = toScreen(b.x + (forward ? -boxW / 2 : boxW / 2), b.y);
      const hot = hover?.node === wire.from || hover?.node === wire.to;
      const direct = trace.chain || wire.from === selected?.node || wire.to === selected?.node;
      context.globalAlpha = hot ? 1 : hover?.node !== undefined ? .16 : direct ? .8 : .36;
      context.strokeStyle = hot ? "#ffffff" : trace.chain ? (wire.back ? amber : cyan)
        : wire.from === selected?.node ? cyan : wire.to === selected?.node ? amber : "#7089ad";
      context.lineWidth = hot || trace.chain ? 2 : 1.2;
      const reach = Math.max(30, Math.abs(bx - ax) / 2) * (forward ? 1 : -1);
      context.beginPath();
      context.moveTo(ax, ay);
      context.bezierCurveTo(ax + reach, ay, bx - reach, by, bx, by);
      context.stroke();
      if ((hot || (direct && trace.wires.length < 60)) && s > .5) {
        context.globalAlpha = 1;
        text(wire.label, (ax + bx) / 2, (ay + by) / 2 - 4, 10.5, hot ? "#ffffff" : "#b9c8dc", "center", 500, true);
      }
    }
    context.globalAlpha = 1;
    for (const [node, at] of trace.place) {
      const [x, y] = toScreen(at.x - boxW / 2, at.y - boxH / 2);
      const chosen = node === selected?.node;
      context.fillStyle = chosen ? "#1d3140" : hover?.node === node ? "#18263a" : "#101826";
      context.strokeStyle = chosen ? "#ffffff" : nodeIssues.has(node) ? red : "#33455e";
      context.lineWidth = chosen ? 1.8 : 1;
      context.beginPath();
      context.rect(x, y, boxW * s, boxH * s);
      context.fill();
      context.stroke();
      context.fillStyle = colour(node);
      context.fillRect(x, y, 3.5 * s, boxH * s);
      if (pins.includes(node)) { context.fillStyle = cyan; context.fillRect(x + (boxW - 9) * s, y + 4 * s, 5 * s, 5 * s); }
      if (s < .3) continue;
      context.textAlign = "left";
      context.font = "650 " + 13 * s + "px system-ui, 'Segoe UI', sans-serif";
      context.fillStyle = "#f1f6fc";
      context.fillText(clip(graph.nodes[node].label, 27), x + 12 * s, y + 18 * s);
      context.font = "500 " + 9.5 * s + "px ui-monospace, 'Cascadia Mono', Consolas, monospace";
      context.fillStyle = colour(node);
      context.fillText(clip((kindOf(node) === "code" ? "SRC · " : "") + catalogs[graph.nodes[node].c].label.toUpperCase(), 32), x + 12 * s, y + 33 * s);
    }
    for (const extra of trace.extras) {
      const [x, y] = toScreen(extra.x, extra.y);
      text(extra.text, x, y, Math.max(9, 11 * s), "#8ea2bd", "center", 500, true);
    }
    const top = Math.min(...[...trace.place.values()].map(p => p.y)) - boxH * 1.15;
    for (const [col, label] of trace.heads) {
      if (![...trace.place.values()].some(p => p.col === col)) continue;
      const [x, y] = toScreen(col * colGap, top);
      text(label, x, y, Math.max(9, 11.5 * s), trace.chain ? "#ffffff" : col < 0 ? amber : col > 0 ? cyan : "#ffffff", "center", 700, true);
    }
  }

  function draw() {
    const { w, h } = view();
    if (!w || !h) return;
    const ratio = window.devicePixelRatio || 1;
    if (canvas.width !== Math.round(w * ratio) || canvas.height !== Math.round(h * ratio)) {
      canvas.width = Math.round(w * ratio);
      canvas.height = Math.round(h * ratio);
    }
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, w, h);
    context.lineJoin = "round";
    if (mode === "trace" && trace) drawTrace(); else drawMap();
    drawHud();
  }
  function drawHud() {
    const cell = (label, value, tone) => el("span", { className: tone ?? "" }, el("b", {}, label), " " + value);
    const about = selected?.node !== undefined ? clip(graph.nodes[selected.node].label, 26)
      : selected?.catalog !== undefined ? catalogs[selected.catalog].label : "—";
    hud.replaceChildren(
      cell("VIEW", trace?.chain ? "PATH" : mode.toUpperCase()), cell("LENS", lens.toUpperCase()),
      cell("NODES", number(graph.nodes.length)), cell("LINKS", number(graph.edges.length)),
      cell("ISSUES", number((graph.issues ?? []).length), (graph.issues ?? []).length ? "warn" : ""),
      cell("SEL", about), mark ? cell("IMPACT", number(mark.nodes.size), "hot") : "",
      cell("ZOOM", Math.round(camera.s * 100) + "%"));
  }

  // ---- picking -----------------------------------------------------------
  function pick(sx, sy) {
    const [x, y] = toWorld(sx, sy);
    if (mode === "trace" && trace) {
      for (const [node, at] of trace.place) {
        if (Math.abs(x - at.x) <= boxW / 2 && Math.abs(y - at.y) <= boxH / 2) return { node };
      }
      return null;
    }
    const near = mark ? [...mark.nodes.keys()] : selected?.node !== undefined
      ? [...out[selected.node], ...inn[selected.node]].map(row => row[0]) : [];
    for (const other of [...near, ...(selected?.node !== undefined ? [selected.node] : [])]) {
      if (nodeShown(other) && Math.hypot(points[other * 2] - x, points[other * 2 + 1] - y) * camera.s < 8) return { node: other };
    }
    const catalog = catalogs.find(item => shown(item) && Math.hypot(item.x - x, item.y - y) <= item.r);
    if (!catalog) return null;
    if (catalog.spacing * camera.s > 3) {
      let best, range = Math.max(7, catalog.spacing * camera.s * .55) / camera.s;
      for (const node of catalog.members) {
        const distance = Math.hypot(points[node * 2] - x, points[node * 2 + 1] - y);
        if (distance < range) { range = distance; best = node; }
      }
      if (best !== undefined) return { node: best };
    }
    return { catalog: catalog.index };
  }

  // ---- selection ---------------------------------------------------------
  const same = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  function select(next, travel = true, record = true) {
    if (pathFrom !== null && next?.node !== undefined && next.node !== pathFrom) return finishPath(next.node);
    if (record && !same(next, selected)) { if (selected) past.push(selected); past = past.slice(-40); future = []; }
    selected = next;
    mark = null;
    if (next?.node === undefined && mode === "trace") mode = "map";
    trace = mode === "trace" && next?.node !== undefined ? buildTrace(next.node) : null;
    refreshChrome();
    hooks.onSelect?.(specOf(next));
    if (mode === "trace") return fit(true);
    if (!travel) return draw();
    if (next?.node !== undefined) {
      const group = catalogs[graph.nodes[next.node].c].group;
      if (hiddenGroups.has(group)) { hiddenGroups.delete(group); drawLayers(); }
      frameNodes([next.node, ...out[next.node].map(row => row[0]), ...inn[next.node].map(row => row[0])].filter(nodeShown));
    } else if (next?.catalog !== undefined) {
      const catalog = catalogs[next.catalog];
      if (hiddenGroups.has(catalog.group)) { hiddenGroups.delete(catalog.group); drawLayers(); }
      frame(catalog.x - catalog.r - 12, catalog.y - catalog.r - 12, catalog.x + catalog.r + 12, catalog.y + catalog.r + 12, false,
        Math.max(1.2, 70 / Math.max(1, catalog.spacing)));
    } else fit(true);
  }
  function refreshChrome() {
    depthBox.hidden = mode !== "trace" || Boolean(trace?.chain);
    depthLabel.textContent = depth + (depth === 1 ? " STEP" : " STEPS");
    hint.hidden = pathFrom === null;
    if (pathFrom !== null) hint.textContent = "PATH FROM " + graph.nodes[pathFrom].label.toUpperCase() + " — CLICK OR FIND THE SECOND NODE · ESC CANCELS";
    drawModes();
    drawDock();
    drawSide();
  }
  function setMode(next) {
    if (next === "trace" && selected?.node === undefined) return;
    mode = next;
    select(selected, true, false);
  }
  function setDepth(next) {
    depth = Math.max(1, Math.min(4, next));
    select(selected, true, false);
  }
  function setLens(next) {
    lens = next;
    drawModes();
    draw();
  }
  function step(direction) {
    const from = direction < 0 ? past : future, to = direction < 0 ? future : past;
    if (!from.length) return;
    if (selected) to.push(selected);
    select(from.pop(), true, false);
  }
  function showImpact() {
    if (selected?.node === undefined) return;
    const origin = selected.node;
    if (mode === "trace") { mode = "map"; trace = null; }
    mark = { title: graph.nodes[origin].label, nodes: impact(adj, origin, graph) };
    refreshChrome();
    frameNodes([origin, ...mark.nodes.keys()].filter(nodeShown), 4);
  }
  function startPath() {
    if (selected?.node === undefined) return;
    pathFrom = selected.node;
    refreshChrome();
    finder.focus();
  }
  function finishPath(target) {
    const origin = pathFrom;
    pathFrom = null;
    const chain = shortestPath(adj, origin, target);
    if (!chain) {
      hooks.note?.("No chain of links connects these two nodes.", true);
      return select({ node: target });
    }
    if (selected) past.push(selected);
    selected = { node: target };
    mark = null;
    mode = "trace";
    trace = buildChain(chain);
    refreshChrome();
    hooks.onSelect?.(specOf(selected));
    fit(true);
  }
  function togglePin(node = selected?.node) {
    if (node === undefined) return;
    if (pins.includes(node)) pins = pins.filter(item => item !== node);
    else if (pins.length >= maxPins) return hooks.note?.("The agent takes " + maxPins + " sources at most. Remove a pin first.", true);
    else pins = [...pins, node];
    refreshChrome();
    draw();
  }

  // ---- chrome ------------------------------------------------------------
  const toggle = (label, on, run, title) => {
    const item = button(label, run, title);
    item.setAttribute("aria-pressed", String(on));
    return item;
  };
  function drawModes() {
    modeBar.replaceChildren(
      toggle("MAP", mode === "map", () => setMode("map"), "Whole game"),
      toggle("TRACE", mode === "trace" && !trace?.chain, () => setMode("trace"), "Chain of the selected node in columns (T)"));
    modeBar.children[1].disabled = selected?.node === undefined;
    lensBar.replaceChildren(el("span", {}, "LENS"),
      toggle("AREA", lens === "area", () => setLens("area"), "Colour by game area"),
      toggle("KIND", lens === "kind", () => setLens("kind"), "Records, catalog files and source files"),
      toggle("HEAT", lens === "heat", () => setLens("heat"), "Brighter means more links"),
      toggle("NET", lens === "net", () => setLens("net"), "Multiplayer: which world state reaches joining players"),
      toggle("ISSUES " + (graph.issues ?? []).length, lens === "issues", () => setLens("issues"), "Broken links, duplicates and unused records"));
  }
  function drawLayers() {
    const flip = names => {
      const allHidden = names.every(name => hiddenGroups.has(name));
      for (const name of names) if (allHidden) hiddenGroups.delete(name); else hiddenGroups.add(name);
      drawLayers();
      fit(true);
    };
    const data = groups.filter(group => !group.code).map(group => group.name), code = groups.filter(group => group.code).map(group => group.name);
    layerBar.replaceChildren(
      toggle("DATA", data.some(name => !hiddenGroups.has(name)), () => flip(data), "Show or hide all catalog areas"),
      toggle("CODE", code.some(name => !hiddenGroups.has(name)), () => flip(code), "Show or hide Rust source"),
      ...groups.map(group => {
        const item = toggle(group.name.replace("Code · ", "").toUpperCase(), !hiddenGroups.has(group.name), () => flip([group.name]), group.name + " · " + group.nodes + " nodes");
        item.style.setProperty("--chip", tint(group.name));
        item.className = "game-layer";
        return item;
      }));
  }
  function drawDock() {
    dock.hidden = !pins.length;
    if (!pins.length) return;
    dock.replaceChildren(el("b", {}, "PINNED " + pins.length + "/" + maxPins),
      ...pins.map(node => {
        const chip = el("span", { className: "game-pin", style: "--chip:" + colour(node) },
          button(clip(graph.nodes[node].label, 22), () => select({ node }), "Select"), button("×", () => togglePin(node), "Remove pin"));
        return chip;
      }),
      button("ASK AGENT", () => hooks.assist(pins.map(nodeSpec),
        "Use these graph nodes and their links as context.\n\n" + brief(graph, adj, pins) + "\nRequested change: "), "Open the agent with these sources and their links", "primary"),
      button("COPY BRIEF", async () => {
        try { await navigator.clipboard.writeText(brief(graph, adj, pins)); hooks.note?.("Copied a Markdown brief of " + pins.length + " nodes."); }
        catch { hooks.note?.("The browser blocked the clipboard.", true); }
      }, "Copy sources, links and blast radius as Markdown"),
      button("CLEAR", () => { pins = []; refreshChrome(); draw(); }, "Remove every pin"));
  }

  const section = (title, ...body) => el("section", { className: "game-section" }, el("h3", {}, title), ...body);
  const chip = (group, label) => el("span", { className: "game-chip", style: "--chip:" + tint(group) }, label);
  function row(label, detail, group, run, title, flagged) {
    const entry = button("", run, title ?? label, "game-row" + (flagged ? " flagged" : ""));
    entry.append(el("i", { style: "background:" + (flagged ? red : tint(group)) }), el("strong", {}, label), el("small", {}, detail));
    return entry;
  }
  const nodeRow = (node, detail) => row(graph.nodes[node].label, detail, catalogs[graph.nodes[node].c].group,
    () => select({ node }), nodeSpec(node).path, nodeIssues.has(node));
  function grouped(list, detailOf) {
    const byCatalog = new Map();
    for (const item of list) {
      const catalog = graph.nodes[item[0]].c;
      if (!byCatalog.has(catalog)) byCatalog.set(catalog, []);
      byCatalog.get(catalog).push(item);
    }
    return [...byCatalog].sort((a, b) => Number(catalogs[a[0]].kind === "code") - Number(catalogs[b[0]].kind === "code") || b[1].length - a[1].length)
      .map(([catalog, items]) => {
        const body = el("div", { className: "game-rows" });
        const fill = all => body.replaceChildren(...items.slice(0, all ? 400 : 8).map(item => nodeRow(item[0], detailOf(item))),
          items.length > 8 && !all ? button("+ " + (items.length - 8) + " MORE", () => fill(true), "", "game-more") : "");
        fill(false);
        const head = button("", () => select({ catalog }), "Select this cluster", "game-group-head");
        head.append(chip(catalogs[catalog].group, (catalogs[catalog].kind === "code" ? "SRC · " : "") + catalogs[catalog].label), el("span", {}, String(items.length)));
        return el("div", { className: "game-group" }, head, body);
      });
  }
  const stat = (label, value, tone) => el("div", { className: "game-stat " + (tone ?? "") }, el("b", {}, String(value)), el("span", {}, label));
  function header(kicker, title, path) {
    const nav = el("div", { className: "game-nav" },
      button("‹", () => step(-1), "Previous selection (Backspace)"), button("›", () => step(1), "Next selection"),
      selected ? button("×", () => select(null), "Clear selection (Esc)") : "");
    nav.children[0].disabled = !past.length;
    nav.children[1].disabled = !future.length;
    return el("header", { className: "game-head" }, el("div", {}, el("span", { className: "game-kicker" }, kicker), nav),
      el("h2", {}, title), path ? el("code", { title: "Click to copy", onclick: () => navigator.clipboard?.writeText(path).then(() => hooks.note?.("Copied " + path)) }, path) : null);
  }
  function fieldsSection(spec) {
    if (!hooks.fields) return "";
    const body = el("div", { className: "game-fields" }, el("p", { className: "muted" }, "Reading…"));
    const ticket = spec.path + JSON.stringify(spec.trail ?? "");
    body.dataset.ticket = ticket;
    hooks.fields(spec).then(rows => {
      if (body.dataset.ticket !== ticket) return;
      const fill = all => body.replaceChildren(...(rows.length ? rows.slice(0, all ? 200 : 12).map(([key, value]) =>
        el("div", {}, el("span", {}, key), el("code", { title: value }, value))) : [el("p", { className: "muted" }, "No values to show.")]),
      rows.length > 12 && !all ? button("+ " + (rows.length - 12) + " MORE", () => fill(true), "", "game-more") : "");
      fill(false);
    }).catch(error => body.replaceChildren(el("p", { className: "error" }, error.message)));
    return section(spec.kind === "code" ? "TUNABLE CONSTANTS" : "VALUES", body);
  }
  function issueRows(list) {
    return list.slice(0, 60).map(issue => row(issue.text, issueNames[issue.type], "Settings",
      () => select(issue.node !== undefined ? { node: issue.node } : { catalog: issue.catalog }), issue.text, true))
      .map(entry => { entry.classList.add("wrap"); return entry; });
  }

  function drawSide() {
    const body = el("div", { className: "game-body" });
    if (selected?.node !== undefined) {
      const index = selected.node, node = graph.nodes[index], catalog = catalogs[node.c], spec = nodeSpec(index), kind = kindOf(index);
      const reach = impact(adj, index, graph);
      const pinned = pins.includes(index);
      body.append(
        el("div", { className: "game-actions" },
          button("EDIT", () => hooks.edit(spec), "Open fields and controls (Enter)", "primary"),
          toggle("TRACE", mode === "trace" && !trace?.chain, () => setMode(mode === "trace" ? "map" : "trace"), "Chain in columns (T)"),
          toggle("IMPACT", Boolean(mark), () => mark ? select(selected, true, false) : showImpact(), "Mark everything that depends on this (I)"),
          toggle("PATH TO…", pathFrom !== null, () => pathFrom !== null ? (pathFrom = null, refreshChrome()) : startPath(), "Find the chain to another node (P)"),
          toggle(pinned ? "PINNED" : "PIN", pinned, () => togglePin(), "Collect for the agent (Space)"),
          button("OPEN", () => hooks.openFile(spec.path, node.at), "Open " + spec.path),
          button("⋯", event => hooks.menu(spec, event.currentTarget), "Notes, boards, wiring and AI")),
        el("div", { className: "game-stats" }, stat("USES", out[index].length, "cyan"), stat("USED BY", inn[index].length, "amber"),
          stat("IMPACT", reach.size, reach.size > 40 ? "hot" : ""),
          kind === "code" ? stat("LINES", number(node.lines ?? 0)) : stat("IN CODE", node.code ?? 0)),
        nodeIssues.has(index) ? section("ISSUES", el("div", { className: "game-rows" }, ...issueRows(nodeIssues.get(index)))) : "",
        mark ? section("IMPACT · " + mark.nodes.size + " DEPENDANTS", ...(mark.nodes.size
          ? grouped([...mark.nodes], item => item[1] === 1 ? "direct" : item[1] + " steps")
          : [el("p", { className: "muted" }, "Nothing depends on this node.")])) : "",
        trace?.chain ? section("PATH · " + (trace.chain.length - 1) + " LINKS", el("div", { className: "game-rows" },
          ...trace.chain.map((item, order) => nodeRow(item, order ? trace.wires[order - 1].label : "start")))) : "",
        kind === "catalog" ? "" : fieldsSection(spec),
        section("USES · " + out[index].length, ...(out[index].length ? grouped(out[index], item => item[1])
          : [el("p", { className: "muted" }, "Points at nothing else.")])),
        section("USED BY · " + inn[index].length, ...(inn[index].length ? grouped(inn[index], item => item[1])
          : [el("p", { className: "muted" }, kind === "record" && node.code ? "No direct link. " + node.code + " source files name it." : "Nothing refers to this node.")])));
      side.replaceChildren(header((kind === "code" ? "SOURCE" : kind === "catalog" ? "CATALOG FILE" : "RECORD") + " · " + catalog.label.toUpperCase(),
        node.label, spec.path), body);
      return;
    }
    if (selected?.catalog !== undefined) {
      const catalog = catalogs[selected.catalog], related = catalogLinks(catalog.index);
      const filter = el("input", { type: "search", placeholder: "FILTER " + catalog.members.length + " NODES", "aria-label": "Filter nodes" });
      const list = el("div", { className: "game-rows" });
      const fill = () => {
        const words = filter.value.toLowerCase();
        const hits = catalog.members.filter(node => graph.nodes[node].label.toLowerCase().includes(words))
          .sort((a, b) => Number(Boolean(graph.nodes[b].hub)) - Number(Boolean(graph.nodes[a].hub)) || degree(b) - degree(a));
        list.replaceChildren(...hits.slice(0, 80).map(node => nodeRow(node, out[node].length + " → · ← " + inn[node].length)),
          hits.length > 80 ? el("p", { className: "muted" }, (hits.length - 80) + " more. Type to narrow.") : "");
      };
      filter.oninput = fill;
      fill();
      const links = rows => el("div", { className: "game-rows" }, ...rows.slice(0, 30).map(([other, count]) =>
        row((catalogs[other].kind === "code" ? "src · " : "") + catalogs[other].label, count + (count === 1 ? " link" : " links"), catalogs[other].group, () => select({ catalog: other }))));
      const flagged = catalog.members.flatMap(node => nodeIssues.get(node) ?? []).concat(catalogIssues.get(catalog.index) ?? []);
      body.append(
        el("div", { className: "game-actions" },
          button(catalog.kind === "code" ? "BROWSE" : "EDIT ROWS", () => hooks.edit(catalogSpec(catalog)), "Open rows and controls", "primary"),
          catalog.kind === "code" ? "" : button("OPEN", () => hooks.openFile(catalog.path), catalog.path),
          button("⋯", event => hooks.menu(catalogSpec(catalog), event.currentTarget), "Notes, boards and AI")),
        el("div", { className: "game-stats" }, stat(catalog.kind === "code" ? "FILES" : "RECORDS", catalog.count),
          stat("USES", related.uses.length, "cyan"), stat("USED BY", related.usedBy.length, "amber"), stat("ISSUES", flagged.length, flagged.length ? "hot" : "")),
        catalog.error ? el("p", { className: "error" }, "Could not read this catalog: " + catalog.error) : "",
        flagged.length ? section("ISSUES", el("div", { className: "game-rows" }, ...issueRows(flagged))) : "",
        section(catalog.kind === "code" ? "FILES" : "NODES", filter, list),
        related.uses.length ? section("USES", links(related.uses)) : "",
        related.usedBy.length ? section("USED BY", links(related.usedBy)) : "");
      side.replaceChildren(header((catalog.kind === "code" ? "SOURCE MODULE" : "CATALOG") + " · " + catalog.group.toUpperCase(), catalog.label, catalog.path), body);
      return;
    }
    const issues = graph.issues ?? [];
    const kinds = { record: 0, catalog: 0, code: 0 };
    graph.nodes.forEach((_, index) => kinds[kindOf(index)]++);
    const hubs = graph.nodes.map((_, index) => index).filter(index => kindOf(index) === "record")
      .sort((a, b) => degree(b) - degree(a)).slice(0, 12);
    const byType = new Map();
    for (const issue of issues) byType.set(issue.type, [...(byType.get(issue.type) ?? []), issue]);
    body.append(
      el("div", { className: "game-stats" }, stat("RECORDS", number(kinds.record)), stat("CATALOGS", kinds.catalog),
        stat("SOURCE", number(kinds.code)), stat("LINKS", number(graph.edges.length), "cyan")),
      el("p", { className: "game-lead" }, "Circles are catalog files and source modules. Dots are records, squares are Rust files. Every line is a reference read from the data or the code."),
      el("div", { className: "game-keys" }, ...[["CLICK", "select"], ["DBL-CLICK", "trace chain"], ["/", "find"], ["T", "trace"], ["I", "impact"],
        ["P", "path to…"], ["SPACE", "pin for agent"], ["BKSP", "back"], ["0", "fit"], ["ESC", "clear"]]
        .map(([key, what]) => el("span", {}, el("kbd", {}, key), what))),
      section("ISSUES · " + issues.length, ...(issues.length ? [...byType].map(([type, list]) => {
        const rows = el("div", { className: "game-rows" }, ...issueRows(list));
        return el("details", { className: "game-details", open: type === "dangling" || type === "duplicate" },
          el("summary", {}, issueNames[type] + " · " + list.length), rows);
      }) : [el("p", { className: "muted" }, "No broken links, duplicates or unused records found.")])),
      (graph.net ?? []).length ? section("MULTIPLAYER COVERAGE · " + graph.net.filter(item => item.status !== "local" || item.note?.ok).length + "/" + graph.net.length,
        el("details", { className: "game-details", open: lens === "net" }, el("summary", {}, "Shared world state and whether joiners receive it"),
          el("div", { className: "game-rows" }, ...[...graph.net].sort((a, b) => Number(b.status === "local" && !b.note?.ok) - Number(a.status === "local" && !a.note?.ok)).map(item => {
            const gap = item.status === "local" && !item.note?.ok;
            const entry = row("World." + item.field + (item.note ? " — " + item.note.why : ""),
              gap ? "NOT SENT" : item.note?.ok ? "OK · REVIEWED" : item.status.toUpperCase(), "Settings",
              () => { setLens("net"); if (item.node !== undefined) select({ node: item.node }); }, item.type + " · " + (item.files.join(", ") || "no multiplayer source touches it"), gap);
            entry.classList.add("wrap");
            if (!gap) entry.querySelector("i").style.background = netTone[item.status === "local" ? "replicated" : item.status];
            return entry;
          })))) : "",
      section("MOST CONNECTED RECORDS", el("div", { className: "game-rows" }, ...hubs.map(node =>
        nodeRow(node, catalogs[graph.nodes[node].c].label + " · " + degree(node))))),
      section("INFERRED LINK FIELDS · " + (graph.schema ?? []).length, el("details", { className: "game-details" },
        el("summary", {}, "How records were connected"),
        el("div", { className: "game-fields" }, ...(graph.schema ?? []).map(rule =>
          el("div", {}, el("span", {}, rule.from + "." + rule.key.replace(/^P:/, "") ), el("code", {}, "→ " + rule.to.join(", "))))))));
    side.replaceChildren(header("WHOLE GAME · LIVE FROM SOURCE", "Axiom", null), body);
  }

  // ---- finder ------------------------------------------------------------
  function find() {
    const query = finder.value.trim();
    if (!query) { finderList.hidden = true; return; }
    const words = query.toLowerCase().split(/\s+/);
    const clusters = catalogs.filter(catalog => words.every(word => catalog.label.toLowerCase().includes(word) || catalog.id.includes(word))).slice(0, 4);
    const nodes = findNodes(graph, query, 16);
    const pickHit = hit => {
      finderList.hidden = true;
      finder.value = "";
      canvas.focus();
      select(hit);
    };
    finderList.replaceChildren(
      ...clusters.map(catalog => row(catalog.label, (catalog.kind === "code" ? "module · " : "catalog · ") + catalog.count, catalog.group, () => pickHit({ catalog: catalog.index }))),
      ...nodes.map(node => row(graph.nodes[node].label, (kindOf(node) === "code" ? "src · " : "") + catalogs[graph.nodes[node].c].label,
        catalogs[graph.nodes[node].c].group, () => pickHit({ node }), nodeSpec(node).path, nodeIssues.has(node))),
      clusters.length + nodes.length ? "" : el("p", { className: "muted" }, "Nothing matches. Ctrl+K also searches settings and notes."));
    finderList.hidden = false;
  }
  finder.oninput = find;
  finder.onkeydown = event => {
    if (event.key === "Enter") { event.preventDefault(); finderList.querySelector("button")?.click(); }
    if (event.key === "ArrowDown") { event.preventDefault(); finderList.querySelector("button")?.focus(); }
    if (event.key === "Escape") { finderList.hidden = true; finder.value = ""; canvas.focus(); }
  };
  finderList.onkeydown = event => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    (event.key === "ArrowDown" ? document.activeElement.nextElementSibling : document.activeElement.previousElementSibling ?? finder)?.focus();
  };

  // ---- input -------------------------------------------------------------
  const local = event => {
    const bounds = canvas.getBoundingClientRect();
    return [event.clientX - bounds.left, event.clientY - bounds.top];
  };
  canvas.addEventListener("wheel", event => {
    event.preventDefault();
    zoomBy(Math.exp(-event.deltaY * .0016), ...local(event));
  }, { passive: false });
  canvas.onpointerdown = event => {
    if (event.button !== 0) return;
    drag = { x: event.clientX, y: event.clientY, cx: camera.x, cy: camera.y };
    moved = false;
    canvas.setPointerCapture(event.pointerId);
  };
  canvas.onpointermove = event => {
    if (drag) {
      if (Math.hypot(event.clientX - drag.x, event.clientY - drag.y) > 4) moved = true;
      if (!moved) return;
      flight = undefined;
      camera.x = drag.cx - (event.clientX - drag.x) / camera.s;
      camera.y = drag.cy - (event.clientY - drag.y) / camera.s;
      canvas.classList.add("dragging");
      return draw();
    }
    const [x, y] = local(event);
    const next = pick(x, y);
    if (!same(next, hover)) { hover = next; draw(); }
    canvas.style.cursor = next ? "pointer" : "";
    if (!next) { tip.hidden = true; return; }
    const catalog = catalogs[next.catalog ?? graph.nodes[next.node].c];
    const flags = next.node !== undefined ? nodeIssues.get(next.node) : catalogIssues.get(next.catalog);
    tip.replaceChildren(el("b", {}, next.node !== undefined ? graph.nodes[next.node].label : catalog.label),
      el("span", {}, next.node !== undefined
        ? catalog.label + " · " + out[next.node].length + " → · ← " + inn[next.node].length
        : catalog.count + (catalog.kind === "code" ? " files · " : " records · ") + catalog.path),
      flags ? el("em", {}, flags[0].text) : "");
    tip.style.left = Math.min(x + 14, view().w - 310) + "px";
    tip.style.top = y + 16 + "px";
    tip.hidden = false;
  };
  canvas.onpointerup = event => {
    const wasDrag = moved;
    drag = undefined;
    canvas.classList.remove("dragging");
    if (wasDrag || event.button !== 0) return;
    const hit = pick(...local(event));
    if (event.shiftKey && hit?.node !== undefined) return togglePin(hit.node);
    if (!same(hit, selected)) select(hit, mode === "trace" || hit?.catalog !== undefined);
  };
  canvas.onpointerleave = () => { tip.hidden = true; if (hover) { hover = null; draw(); } };
  canvas.ondblclick = event => {
    const hit = pick(...local(event));
    if (hit?.node !== undefined) { selected = hit; setMode("trace"); }
  };
  canvas.oncontextmenu = event => {
    const hit = pick(...local(event));
    if (!hit) return;
    event.preventDefault();
    hooks.menu(specOf(hit), canvas, [event.clientX, event.clientY]);
  };
  element.addEventListener("keydown", event => {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.ctrlKey || event.metaKey || event.altKey) return;
    const key = event.key.toLowerCase();
    if (key === "/") finder.focus();
    else if (key === "+" || key === "=") zoomBy(1.3);
    else if (key === "-") zoomBy(1 / 1.3);
    else if (key === "0") fit(true);
    else if (key === "t") setMode(mode === "trace" ? "map" : "trace");
    else if (key === "i") showImpact();
    else if (key === "p") startPath();
    else if (key === " " && event.target === canvas) togglePin();
    else if (key === "[") setDepth(depth - 1);
    else if (key === "]") setDepth(depth + 1);
    else if (key === "backspace") step(-1);
    else if (key === "enter" && event.target === canvas && selected) hooks.edit(specOf(selected));
    else if (key === "escape") {
      if (pathFrom !== null) { pathFrom = null; refreshChrome(); }
      else if (mark) select(selected, false, false);
      else if (mode === "trace") setMode("map");
      else select(null);
    } else return;
    event.preventDefault();
  });
  new ResizeObserver(() => {
    if (!canvas.clientWidth) return;
    if (!fitted) { fitted = true; fit(false); } else draw();
  }).observe(canvas);

  drawLayers();
  select(null, false, false);
  return {
    element, stamp: graph.stamp, trace: () => setMode("trace"), impact: showImpact, lens: setLens,
    current: () => specOf(selected),
    focus(path, trail) {
      const key = JSON.stringify(trail ?? []);
      const file = graph.nodes.findIndex(node => node.file === path);
      if (file >= 0) { select({ node: file }); return true; }
      const catalog = catalogs.find(item => item.path === path);
      if (!catalog) return false;
      const node = trail?.length ? catalog.members.find(member => JSON.stringify(graph.nodes[member].at) === key) : undefined;
      select(node !== undefined ? { node } : { catalog: catalog.index });
      return true;
    },
  };
}
