// The browser and AI command interface use these exact operations.
export const portalPath = "assets/portal_layout.ron";
export const dimensions = ["Surface", "Hell", "Manyfold"];
const object = value => value && typeof value === "object" && !Array.isArray(value);
function keys(value, allowed, required = []) {
  if (!object(value) || Object.keys(value).some(key => !allowed.includes(key)) || required.some(key => !Object.hasOwn(value, key)))
    throw new Error("Unexpected or missing operation fields.");
}
function pick(rows, id, label) {
  const found = rows.filter(row => row.id === id);
  if (found.length !== 1) throw new Error("Choose one existing " + label + ": " + id);
  return found[0];
}
const nodeFields = ["label", "dimension", "position", "yaw_degrees", "half_size", "canvas"];
const linkFields = ["two_way", "enabled"];
export const portalOperations = {
  add_node: "Add an opening: node {id,label,dimension,position:[x,y,z],yaw_degrees,half_size:[w/2,h/2],canvas:[x,y]}",
  update_node: "Update an opening: id, changes (stable ID cannot change)",
  remove_node: "Remove an unlinked opening: id (remove its links first)",
  connect: "Connect openings: link {id,kind:Fold,from,to,two_way,enabled}",
  update_link: "Change link direction or availability: id, changes {two_way,enabled}",
  remove_link: "Remove a link: id",
};
export function applyPortalOperations(original, operations) {
  if (!Array.isArray(operations) || operations.length > 128) throw new Error("Use up to 128 operations per revision.");
  const graph = structuredClone(original);
  for (const [index, operation] of operations.entries()) {
    try {
      if (!object(operation) || !Object.hasOwn(portalOperations, operation.op)) throw new Error("Unknown portal operation.");
      switch (operation.op) {
        case "add_node":
          keys(operation, ["op", "node"], ["node"]);
          keys(operation.node, ["id", ...nodeFields], ["id", ...nodeFields]);
          if (graph.nodes.some(node => node.id === operation.node.id)) throw new Error("Opening ID already exists.");
          graph.nodes.push(structuredClone(operation.node)); break;
        case "update_node":
          keys(operation, ["op", "id", "changes"], ["id", "changes"]);
          keys(operation.changes, nodeFields);
          Object.assign(pick(graph.nodes, operation.id, "opening"), structuredClone(operation.changes)); break;
        case "remove_node":
          keys(operation, ["op", "id"], ["id"]);
          pick(graph.nodes, operation.id, "opening");
          if (graph.links.some(link => link.from === operation.id || link.to === operation.id)) throw new Error("Remove this opening's links first.");
          graph.nodes = graph.nodes.filter(node => node.id !== operation.id); break;
        case "connect": {
          keys(operation, ["op", "link"], ["link"]);
          keys(operation.link, ["id", "kind", "from", "to", ...linkFields], ["id", "kind", "from", "to", ...linkFields]);
          const { from, to, id, kind } = operation.link;
          if (kind !== "Fold") throw new Error("Choose a supported Fold link.");
          if (from === to) throw new Error("Connect two different openings.");
          if (pick(graph.nodes, from, "opening").dimension !== pick(graph.nodes, to, "opening").dimension)
            throw new Error("Fold links stay in one dimension. Hell realm transfer uses the existing built-in gates.");
          if (graph.links.some(link => link.id === id)) throw new Error("Link ID already exists.");
          graph.links.push(structuredClone(operation.link)); break;
        }
        case "update_link":
          keys(operation, ["op", "id", "changes"], ["id", "changes"]);
          keys(operation.changes, linkFields);
          Object.assign(pick(graph.links, operation.id, "link"), structuredClone(operation.changes)); break;
        case "remove_link":
          keys(operation, ["op", "id"], ["id"]);
          pick(graph.links, operation.id, "link");
          graph.links = graph.links.filter(link => link.id !== operation.id); break;
      }
    } catch (error) { throw new Error("Operation " + (index + 1) + ": " + error.message); }
  }
  return graph;
}
export function encodePortalLayout(graph) {
  const number = value => { if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("Enter finite numeric coordinates and sizes."); return String(value); };
  const tuple = (value, count) => {
    if (!Array.isArray(value) || value.length !== count) throw new Error("Invalid coordinate shape.");
    return "(" + value.map(number).join(", ") + ")";
  };
  const string = value => { if (typeof value !== "string") throw new Error("Use text for IDs and labels."); return JSON.stringify(value); };
  const bool = value => { if (typeof value !== "boolean") throw new Error("Use true or false for link rules."); return String(value); };
  const nodes = graph.nodes.map(node => {
    if (!dimensions.includes(node.dimension)) throw new Error("Choose Surface, Hell, or Manyfold.");
    return `    (id: ${string(node.id)}, label: ${string(node.label)}, dimension: ${node.dimension}, position: ${tuple(node.position, 3)}, yaw_degrees: ${number(node.yaw_degrees)}, half_size: ${tuple(node.half_size, 2)}, canvas: ${tuple(node.canvas, 2)}),`;
  });
  const links = graph.links.map(link => {
    if (link.kind !== "Fold") throw new Error("Unsupported link kind.");
    return `    (id: ${string(link.id)}, kind: Fold, from: ${string(link.from)}, to: ${string(link.to)}, two_way: ${bool(link.two_way)}, enabled: ${bool(link.enabled)}),`;
  });
  return `// Forge portal layout. Centres and aperture sizes use metres. Rebuild to activate.\n(version: ${number(graph.version)},\n  nodes: [\n${nodes.join("\n")}\n  ],\n  links: [\n${links.join("\n")}\n  ],\n)\n`;
}
