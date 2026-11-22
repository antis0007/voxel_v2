// Pure queries over the whole-game graph. Shared by the browser map and the
// command line, so a person and an agent get the same answers.

export function adjacency(graph) {
  const out = graph.nodes.map(() => []), inn = graph.nodes.map(() => []);
  for (const [from, to, label] of graph.edges) {
    out[from].push([to, graph.labels[label]]);
    inn[to].push([from, graph.labels[label]]);
  }
  return { out, inn };
}

// Everything that depends on `start`, directly or through other records.
// Source files end a chain: the code naming a record is affected, the code
// importing that code is not counted.
export function impact(adj, start, graph, limit = 6) {
  const depth = new Map([[start, 0]]);
  let frontier = [start];
  const fromCode = Boolean(graph.nodes[start].file);
  for (let step = 1; step <= (fromCode ? 2 : limit) && frontier.length; step++) {
    const next = [];
    for (const node of frontier) {
      if (node !== start && !fromCode && graph.nodes[node].file) continue;
      for (const [user] of adj.inn[node]) {
        if (!depth.has(user)) { depth.set(user, step); next.push(user); }
      }
    }
    frontier = next;
  }
  depth.delete(start);
  return depth;
}

// Shortest chain between two nodes, following links in either direction.
export function shortestPath(adj, from, to) {
  if (from === to) return [from];
  const prior = new Map([[from, -1]]);
  let frontier = [from];
  while (frontier.length) {
    const next = [];
    for (const node of frontier) {
      for (const [other] of [...adj.out[node], ...adj.inn[node]]) {
        if (prior.has(other)) continue;
        prior.set(other, node);
        if (other === to) {
          const chain = [to];
          while (chain[0] !== from) chain.unshift(prior.get(chain[0]));
          return chain;
        }
        next.push(other);
      }
    }
    frontier = next;
  }
  return null;
}

export function linkLabel(adj, a, b) {
  const forward = adj.out[a].find(row => row[0] === b);
  if (forward) return { label: forward[1], forward: true };
  const back = adj.out[b].find(row => row[0] === a);
  return back ? { label: back[1], forward: false } : null;
}

export function findNodes(graph, text, limit = 20) {
  const words = String(text).toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const hits = [];
  if (!graph.degree) {
    const degree = new Uint32Array(graph.nodes.length);
    for (const [from, to] of graph.edges) { degree[from]++; degree[to]++; }
    Object.defineProperty(graph, "degree", { value: degree, enumerable: false });
  }
  graph.nodes.forEach((node, index) => {
    const label = node.label.toLowerCase(), home = graph.catalogs[node.c].label.toLowerCase() + " " + graph.catalogs[node.c].id;
    const key = (node.key ?? "").toLowerCase();
    if (key && key === words.join("")) return void hits.push([70, index]);
    if (!words.every(word => label.includes(word) || home.includes(word) || key.includes(word))) return;
    hits.push([(label === words.join(" ") ? 60 : label.startsWith(words[0]) ? 30 : label.includes(words[0]) ? 12 : 0)
      + (node.hub ? 8 : 0) - (node.file ? 4 : 0) + Math.min(9, graph.degree[index]) / 10, index]);
  });
  return hits.sort((a, b) => b[0] - a[0] || a[1] - b[1]).slice(0, limit).map(hit => hit[1]);
}

export function describeNode(graph, index) {
  const node = graph.nodes[index], home = graph.catalogs[node.c];
  return { index, label: node.label, kind: node.file ? "code" : node.hub ? "catalog" : "record",
    home: home.label, area: home.group, path: node.file ?? home.path, trail: node.at };
}

// Markdown brief an agent can act on without opening the map.
export function brief(graph, adj, picks) {
  const lines = ["# Axiom graph context", ""];
  for (const index of picks) {
    const about = describeNode(graph, index);
    const reach = impact(adj, index, graph);
    lines.push("## " + about.label + " (" + about.kind + " · " + about.home + ")", "- Source: `" + about.path + "`"
      + (about.trail?.length ? " at index path [" + about.trail.join(", ") + "]" : ""));
    const list = (title, rows) => {
      if (!rows.length) return;
      lines.push("- " + title + " (" + rows.length + "):");
      for (const [other, label] of rows.slice(0, 25)) {
        const item = describeNode(graph, other);
        lines.push("  - " + item.label + " — " + item.home + " — via `" + label + "` — `" + item.path + "`");
      }
      if (rows.length > 25) lines.push("  - … " + (rows.length - 25) + " more");
    };
    list("Uses", adj.out[index]);
    list("Used by", adj.inn[index]);
    const files = new Set([...reach.keys()].map(node => describeNode(graph, node).path));
    lines.push("- Blast radius: " + reach.size + " nodes across " + files.size + " files depend on this.", "");
  }
  return lines.join("\n");
}
