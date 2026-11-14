export const recipeId = recipe => JSON.stringify([recipe.path, recipe.parts]);
const natural = code => /^Resource\((?:[0-5]|13)\)$/.test(code);

export function craftingIndex(recipes) {
  const producers = new Map(), consumers = new Map();
  const add = (map, code, recipe) => {
    if (!map.has(code)) map.set(code, []);
    map.get(code).push(recipe);
  };
  for (const recipe of recipes) {
    if (recipe.product && recipe.count > 0) add(producers, recipe.product, recipe);
    for (const { code, count } of recipe.inputs) if (code && count > 0) add(consumers, code, recipe);
  }
  const unpack = recipe => recipe.inputs.length === 1 && recipe.inputs[0].count === 1 && recipe.count > 1 &&
    producers.get(recipe.inputs[0].code)?.some(route =>
      route.count === 1 && route.inputs.length === 1 && route.inputs[0].code === recipe.product &&
      route.inputs[0].count === recipe.count);
  const conversion = new Set(recipes.filter(unpack));
  const productive = new Map([...producers].map(([code, routes]) => [code, routes.filter(route => !conversion.has(route))]));
  const rank = new Map();
  const distance = code => !productive.get(code)?.length || natural(code) ? 0 : rank.get(code) ?? Infinity;
  const cost = recipe => 1 + Math.max(0, ...recipe.inputs.map(input => distance(input.code)));
  for (let pass = 0; pass < producers.size; pass++) {
    let changed = false;
    for (const [code, routes] of productive) {
      const next = Math.min(...routes.map(cost));
      if (next < distance(code)) { rank.set(code, next); changed = true; }
    }
    if (!changed) break;
  }
  for (const routes of producers.values()) routes.sort((a, b) =>
    Number(conversion.has(a)) - Number(conversion.has(b)) || (cost(a) - cost(b) || 0));
  const defaults = new Map();
  for (const [code, routes] of producers) {
    const route = !natural(code) && routes.find(item => !conversion.has(item) && Number.isFinite(cost(item)));
    if (route) defaults.set(code, route);
  }
  return { producers, consumers, defaults };
}

export function planCraft(index, code, count, choices) {
  const surplus = new Map(), supply = new Map(), loops = new Map();
  let total = 0, limited = false;
  const add = (map, item, amount) => map.set(item, (map.get(item) ?? 0) + amount);
  function walk(item, need, path) {
    const node = { item, need, stock: 0, children: [], routes: index.producers.get(item) ?? [] };
    if (++total > 1000 || path.length > 28 || !Number.isSafeInteger(need * 64) || need > 1_000_000_000) {
      limited = true;
      node.status = "Limit";
      return node;
    }
    const have = surplus.get(item) ?? 0;
    node.stock = Math.min(need, have);
    if (node.stock) surplus.set(item, have - node.stock);
    const remaining = need - node.stock;
    if (!remaining) { node.status = "Surplus"; return node; }
    const pick = choices.get(item);
    const selected = pick !== "supply" &&
      (node.routes.find(route => recipeId(route) === pick) ?? index.defaults.get(item));
    if (!selected) {
      add(supply, item, remaining);
      node.status = natural(item) || pick === "supply" || node.routes.length ? "Supply" : "No Recipe";
      return node;
    }
    if (path.includes(item)) {
      add(loops, item, remaining);
      node.status = "Cycle";
      return node;
    }
    node.recipe = selected;
    node.batches = Math.ceil(remaining / selected.count);
    const parents = [...path, item];
    const start = total;
    node.children = selected.inputs.filter(input => input.code && input.count > 0)
      .map(input => walk(input.code, input.count * node.batches, parents));
    if (total <= 1000 && !node.children.some(child => child.incomplete || child.status === "Cycle" || child.status === "Limit")) {
      add(surplus, item, node.batches * selected.count - remaining);
    } else node.incomplete = total > start;
    return node;
  }
  const root = walk(code, count, []);
  return { root, supply, loops, limited };
}
