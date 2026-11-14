import { el, button } from "./inspector.mjs";
import { dragSource, dropTarget } from "./slot-tools.mjs";

const svg = (tag, attrs = {}) => {
  const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  return node;
};

const kindLabel = {
  project: "PROJECT", system: "SYSTEM", folder: "FOLDER", file: "FILE",
  code: "SOURCE", data: "CATALOG", record: "RECORD", field: "TUNABLE",
  rust: "CONSTANT", note: "NOTE", sprite: "SPRITE",
};
export const nodeKind = type => kindLabel[type] ?? type.toUpperCase();

export function graphCanvas(selected, incoming, outgoing, go, offset, page, onContext, onLink) {
  const count = 10;
  const prior = incoming.slice(0, 5);
  const children = outgoing.slice(offset, offset + count);
  const width = 840;
  const rowGap = 97;
  const height = Math.max(390, Math.max(prior.length, Math.min(5, children.length)) * rowGap + 68);
  const middle = height / 2;
  const stage = el("div", { className: "map-stage", tabIndex: 0, "aria-label": "Node graph" });
  const layer = el("div", { className: "map-layer" });
  layer.style.width = width + "px";
  layer.style.height = height + "px";
  const wires = svg("svg", { width, height, "aria-hidden": "true", class: "map-wires" });
  layer.append(wires);
  stage.append(layer);
  const place = (spec, x, y, isCurrent = false) => {
    const attrs = { className: "map-node " + spec.kind + (isCurrent ? " chosen" : ""),
      title: spec.path ?? spec.detail ?? spec.label };
    const node = isCurrent ? el("div", attrs) : el("button", { ...attrs, type: "button", onclick: () => go(spec) });
    if (spec.path && !["folder","system"].includes(spec.kind)) {
      node.setAttribute("aria-label", spec.label);
      if (isCurrent) node.tabIndex = 0;
      node.oncontextmenu = event => { if (onContext?.(spec, node, [event.clientX, event.clientY])) event.preventDefault(); };
      node.onkeydown = event => {
        if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
          if (onContext?.(spec, node)) event.preventDefault();
        }
      };
      if (onLink) {
        dragSource(node, "graph-node", JSON.stringify(spec));
        if (spec.kind === "note") dropTarget(node, ["graph-node"], (_, value) => onLink(JSON.parse(value), spec));
      }
    }
    node.style.left = x + "px";
    node.style.top = y + "px";
    node.append(el("span", { className: "map-kind" }, nodeKind(spec.kind)), el("strong", {}, spec.label));
    if (spec.detail) node.append(el("small", {}, spec.detail));
    layer.append(node);
  };
  const wire = (from, to, kind) => {
    const distance = Math.max(28, (to.x - from.x) / 2);
    wires.append(svg("path", { d: "M " + from.x + " " + from.y + " C " + (from.x + distance) + " "
      + from.y + ", " + (to.x - distance) + " " + to.y + ", " + to.x + " " + to.y, class: "wire " + kind }));
    wires.append(svg("circle", { cx: to.x, cy: to.y, r: 3, class: "port " + kind }));
  };
  const hasPrior = prior.length > 0;
  const centerX = hasPrior ? 235 : 30;
  const centerPort = { x: centerX + 166, y: middle };
  place(selected, centerX, middle, true);
  prior.forEach((spec, index) => {
    const y = middle + (index - (prior.length - 1) / 2) * rowGap;
    wire({ x: 178, y }, { x: centerX, y: middle }, spec.kind);
    place(spec, 12, y);
  });
  children.forEach((spec, index) => {
    const column = index < 5 ? 0 : 1;
    const inColumn = column ? children.length - 5 : Math.min(children.length, 5);
    const x = (hasPrior ? 455 : 255) + column * 196;
    const y = middle + ((index % 5) - (inColumn - 1) / 2) * rowGap + (column ? 31 : 0);
    wire(centerPort, { x, y }, spec.kind);
    place(spec, x, y);
  });
  const frameWidth = children.length > 5 ? (hasPrior ? 840 : 650)
    : children.length ? (hasPrior ? 650 : 450) : centerX + 190;
  let x = 0, y = 0, scale = 1, drag;
  const zoomLabel = el("span", { className: "map-scale" });
  function transform() {
    layer.style.transform = "translate(" + x + "px, " + y + "px) scale(" + scale + ")";
    zoomLabel.textContent = Math.round(scale * 100) + "%";
  }
  function zoom(next, originX = stage.clientWidth / 2, originY = stage.clientHeight / 2) {
    next = Math.max(.35, Math.min(2.5, next));
    x = originX - (originX - x) * next / scale;
    y = originY - (originY - y) * next / scale;
    scale = next;
    transform();
  }
  function fit() {
    if (!stage.clientWidth || !stage.clientHeight) return;
    scale = Math.min(1.14, (stage.clientWidth - 26) / frameWidth, (stage.clientHeight - 16) / height);
    scale = Math.max(.35, scale);
    x = (stage.clientWidth - frameWidth * scale) / 2;
    y = (stage.clientHeight - height * scale) / 2;
    transform();
  }
  stage.addEventListener("wheel", event => {
    event.preventDefault();
    const bounds = stage.getBoundingClientRect();
    zoom(scale * Math.exp(-event.deltaY * .0014), event.clientX - bounds.left, event.clientY - bounds.top);
  }, { passive: false });
  stage.onpointerdown = event => {
    if (event.button !== 0 || event.target.closest("button,.map-node")) return;
    drag = { x: event.clientX - x, y: event.clientY - y };
    stage.setPointerCapture(event.pointerId);
    stage.classList.add("dragging");
  };
  stage.onpointermove = event => {
    if (!drag) return;
    x = event.clientX - drag.x; y = event.clientY - drag.y;
    transform();
  };
  stage.onlostpointercapture = () => {
    drag = undefined;
    stage.classList.remove("dragging");
  };
  stage.onkeydown = event => {
    if (event.target !== stage) return;
    if (event.key === "+" || event.key === "=") zoom(scale * 1.16);
    else if (event.key === "-") zoom(scale / 1.16);
    else if (event.key === "0") fit();
    else return;
    event.preventDefault();
  };

  const prev = button("‹", () => page(Math.max(0, offset - count)), "Previous nodes");
  const next = button("›", () => page(offset + count), "Next nodes");
  prev.disabled = offset === 0;
  next.disabled = offset + count >= outgoing.length;
  const pages = outgoing.length > count ? el("div", { className: "map-controls" }, prev,
    el("span", {}, (offset + 1) + "–" + Math.min(offset + count, outgoing.length) + " / " + outgoing.length), next) : null;
  const controls = el("div", { className: "map-controls" },
    button("−", () => zoom(scale / 1.16), "Zoom out"), zoomLabel,
    button("+", () => zoom(scale * 1.16), "Zoom in"), button("Fit", fit, "Fit nodes to view"));
  requestAnimationFrame(fit);
  return el("div", { className: "map-viewport" }, stage, el("div", { className: "map-bottom" }, controls, pages));
}

