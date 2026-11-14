const svg = (tag, attrs = {}) => {
  const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  return node;
};

function gesture(element, callbacks) {
  element.addEventListener("pointerdown", start => {
    if (start.button !== 0) return;
    const controller = new AbortController();
    const options = { signal: controller.signal, capture: true };
    let moved = false;
    function move(event) {
      if (event.pointerId !== start.pointerId) return;
      if (!moved) {
        if (Math.hypot(event.clientX - start.clientX, event.clientY - start.clientY) < 5) return;
        moved = true;
        element.setPointerCapture(event.pointerId);
        callbacks.begin?.();
      }
      event.preventDefault();
      callbacks.move(event, start);
    }
    function end(event) {
      if (event.pointerId !== start.pointerId) return;
      controller.abort();
      if (!moved) return;
      const preventClick = click => { click.preventDefault(); click.stopImmediatePropagation(); };
      element.addEventListener("click", preventClick, { capture: true, once: true });
      setTimeout(() => element.removeEventListener("click", preventClick, true), 80);
      callbacks.end?.(event, event.type === "pointerup");
    }
    window.addEventListener("pointermove", move, options);
    window.addEventListener("pointerup", end, options);
    window.addEventListener("pointercancel", end, options);
  });
}

export function wireCanvas(surface) {
  const wires = svg("svg", { class: "wiring-lines", "aria-hidden": "true" });
  const lines = svg("g"), preview = svg("path", { class: "wiring-preview" });
  wires.append(lines, preview);
  surface.prepend(wires);
  let entries = [], lit = null;
  // Surface may be CSS-scaled by a zoom view; layout coordinates divide the screen offset by that scale.
  const scale = () => (surface.offsetWidth ? surface.getBoundingClientRect().width / surface.offsetWidth : 1) || 1;
  const point = (x, y) => {
    const bounds = surface.getBoundingClientRect(), k = scale();
    return { x: (x - bounds.left) / k, y: (y - bounds.top) / k };
  };
  const center = element => {
    const bounds = element.getBoundingClientRect();
    return point(bounds.left + bounds.width / 2, bounds.top + bounds.height / 2);
  };
  function curve(a, b) {
    const bend = Math.max(b.x >= a.x ? 6 : 32, Math.min(160, Math.abs(b.x - a.x) * .55));
    return "M " + a.x + " " + a.y + " C " + (a.x + bend) + " " + a.y + ", " +
      (b.x - bend) + " " + b.y + ", " + b.x + " " + b.y;
  }
  function bind(element, anchor, canDrop, drop) {
    let targets = [], over;
    function candidate(event) {
      const node = document.elementFromPoint(event.clientX, event.clientY)?.closest("[data-wire-target]");
      return node && surface.contains(node) && canDrop(node.dataset.wireTarget) ? node : null;
    }
    gesture(element, {
      begin() {
        targets = [...surface.querySelectorAll("[data-wire-target]")].filter(node => canDrop(node.dataset.wireTarget));
        targets.forEach(node => node.classList.add("wire-ready"));
      },
      move(event) {
        const next = candidate(event);
        if (over !== next) over?.classList.remove("wire-over");
        next?.classList.add("wire-over");
        over = next;
        const end = next && next !== surface ? center(next) : point(event.clientX, event.clientY);
        preview.setAttribute("d", curve(center(anchor), end));
      },
      end(event, released) {
        preview.removeAttribute("d");
        over?.classList.remove("wire-over");
        targets.forEach(node => node.classList.remove("wire-ready"));
        const target = released && candidate(event);
        if (target) drop(target.dataset.wireTarget, event);
      },
    });
  }
  function redraw() {
    if (!surface.isConnected) return;
    lines.replaceChildren(...entries.map(edge => {
      const d = curve(center(edge.from), center(edge.to));
      const group = svg("g", { class: edge.output ? "wiring-edge output" : "wiring-edge" });
      group.ends = edge.ends;
      const line = svg("path", { d, class: "wiring-line" });
      const hit = svg("path", { d, class: "wiring-hit" });
      hit.onclick = edge.select;
      if (edge.drop) bind(hit, edge.from, edge.canDrop, edge.drop);
      group.append(line, hit);
      return group;
    }));
    paintLit();
  }
  function paintLit() {
    let any = false;
    for (const group of lines.children) {
      const on = lit !== null && !!group.ends?.includes(lit);
      group.classList.toggle("lit", on);
      any ||= on;
    }
    wires.classList.toggle("has-lit", any);
  }
  /** Light the wires touching one node key (or none) and dim the rest. */
  function highlight(key) {
    const next = key ?? null;
    if (next === lit) return;
    lit = next;
    paintLit();
  }
  function set(edges) {
    entries = edges;
    requestAnimationFrame(redraw);
  }
  function move(node, handle, position, constrain, done) {
    const paint = () => {
      node.style.left = position.x + "px";
      node.style.top = position.y + "px";
    };
    paint();
    let origin;
    gesture(handle, {
      begin() { origin = { ...position }; },
      move(event, start) {
        const k = scale();
        Object.assign(position, constrain(origin.x + (event.clientX - start.clientX) / k, origin.y + (event.clientY - start.clientY) / k));
        paint();
        redraw();
      },
      end(event, released) {
        if (!released) { Object.assign(position, origin); paint(); redraw(); }
        done?.(position, event, released, () => { Object.assign(position, origin); paint(); redraw(); });
      },
    });
  }
  return { set, bind, move, redraw, highlight };
}
