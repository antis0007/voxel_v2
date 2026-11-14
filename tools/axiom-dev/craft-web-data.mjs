import { recipeId } from "./craft-index.mjs";

export function webNeighborhood(index, root, direction, depth, limit = 112) {
  const nodes = new Map(), routes = new Map(), edges = [];
  const itemKey = code => "item:" + code;
  let clipped = false;
  function add(key, spec) {
    if (nodes.has(key)) return true;
    if (nodes.size >= limit) { clipped = true; return false; }
    nodes.set(key, { key, ...spec });
    return true;
  }
  const item = (code, level) => add(itemKey(code), { code, level });
  item(root, 0);
  for (const sign of direction === "both" ? [-1, 1] : [direction === "inputs" ? -1 : 1]) {
    let frontier = [root];
    const visited = new Set();
    for (let hop = 0; hop < depth && frontier.length; hop++) {
      const next = new Set(), level = sign * (hop * 2 + 1);
      for (const code of frontier) {
        if (visited.has(code)) continue;
        visited.add(code);
        const candidates = (sign < 0 ? index.producers : index.consumers).get(code) ?? [];
        for (const route of candidates) {
          const key = "route:" + recipeId(route);
          if (!add(key, { route, level })) continue;
          routes.set(key, route);
          item(route.product, level + 1);
          for (const input of route.inputs) item(input.code, level - 1);
          for (const neighbor of sign < 0 ? route.inputs.map(input => input.code) : [route.product]) {
            if (nodes.has(itemKey(neighbor))) next.add(neighbor);
          }
        }
      }
      frontier = [...next];
    }
  }
  for (const [key, route] of routes) {
    if (nodes.has(itemKey(route.product))) edges.push({ from: key, to: itemKey(route.product), route, output: true });
    for (const input of route.inputs) if (nodes.has(itemKey(input.code))) {
      edges.push({ from: itemKey(input.code), to: key, route, ingredient: input.code, count: input.count });
    }
  }
  return { nodes: [...nodes.values()], edges, clipped };
}

export function checkRoute(index, route, ui) {
  const issues = [];
  if (!route.description?.trim()) issues.push("No recipe description.");
  if (route.inputs.some(input => input.code === route.product)) issues.push("Consumes its own output. Check whether a starter item is required.");
  for (const input of route.inputs) {
    if (!ui.known(input.code)) issues.push("Input not found in the indexed item catalog · " + ui.name(input.code));
    if (!index.producers.has(input.code)) issues.push("External supply needed · " + ui.name(input.code));
  }
  if (!ui.known(route.product)) issues.push("Output not found in the indexed item catalog.");
  if (!ui.art(route.product).length) issues.push("Output has no editable sprite mapping. It may use generated art.");
  const downstream = index.consumers.get(route.product) ?? [];
  const alternatives = (index.producers.get(route.product) ?? []).filter(other => recipeId(other) !== recipeId(route));
  const units = candidate => candidate.inputs.reduce((total, input) => total + input.count, 0) / candidate.count;
  const comparable = alternatives.filter(other => other.inputs.length === route.inputs.length
    && other.inputs.every(input => route.inputs.some(own => own.code === input.code)));
  if (comparable.some(other => units(route) > 2 * units(other))) {
    issues.push("Costs more than twice as many input units per output as another route using the same items.");
  }
  let ceiling = route.count, cycle, searches = 0, limited = false;
  if (route.inputs.length === 1 && route.count > 0 && route.inputs[0].count > 0) {
    const target = route.inputs[0];
    const inspect = (code, ratio, path, visited) => {
      if (++searches > 4000 || path.length > 8) { limited = true; return; }
      if (code === target.code) {
        const safe = Math.floor(target.count / ratio + 1e-9);
        if (safe < ceiling) { ceiling = safe; cycle = path; }
        return;
      }
      for (const next of index.consumers.get(code) ?? []) {
        if (searches > 4000) break;
        if (recipeId(next) === recipeId(route) || next.inputs.length !== 1 || next.inputs[0].count <= 0
            || next.count <= 0 || visited.has(next.product)) continue;
        inspect(next.product, ratio * next.count / next.inputs[0].count, [...path, next.label],
          new Set([...visited, next.product]));
      }
    };
    inspect(route.product, 1, [], new Set([route.product]));
  }
  if (cycle) issues.push("A single-input loop multiplies items · " + [route.label, ...cycle].join(" → "));
  if (limited) issues.push("Conversion search reached its limit. Longer or more complex loops may exist.");
  return { issues, alternatives, downstream, ceiling: cycle ? ceiling : null };
}
