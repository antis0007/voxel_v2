import { el, button } from "./inspector.mjs";
import { catalogIcon } from "./catalog-icons.mjs";
import { showContextMenu } from "./context-menu.mjs";

const targets = new WeakMap();

export function dragSource(element, type, value, extra) {
  element.dataset.draggable = "true";
  element.addEventListener("pointerdown", start => {
    if (start.button !== 0 || element.disabled || element.dataset.draggable !== "true") return;
    const current = typeof value === "function" ? value() : value;
    if (current == null) return;
    const payload = new Map(Object.entries({ [type]: String(current), ...extra }));
    const listeners = new AbortController();
    let ghost, hover, ready = [];
    const controls = { signal: listeners.signal, capture: true };
    function targetAt(event) {
      let found = document.elementFromPoint(event.clientX, event.clientY);
      while (found) {
        const target = targets.get(found);
        const accepted = !found.disabled && target?.types.find(key => payload.has(key));
        if (accepted) return { element: found, target, accepted };
        found = found.parentElement;
      }
    }
    function move(event) {
      if (event.pointerId !== start.pointerId) return;
      if (!ghost) {
        if (Math.hypot(event.clientX - start.clientX, event.clientY - start.clientY) < 6) return;
        const art = element.querySelector("canvas, svg");
        ghost = el("div", { className: "slot-drag-preview", "aria-hidden": "true" },
          element.getAttribute("aria-label") || element.title || element.textContent.trim());
        if (art?.tagName === "CANVAS") {
          const copy = el("canvas", { width: art.width, height: art.height });
          copy.getContext("2d").drawImage(art, 0, 0);
          ghost.prepend(copy);
        } else if (art) ghost.prepend(art.cloneNode(true));
        const scope = element.closest("dialog[open]") ?? document.body;
        ready = [...scope.querySelectorAll("[data-drop-target]")].filter(node =>
          !node.disabled && targets.get(node)?.types.some(key => payload.has(key)));
        for (const node of ready) node.classList.add("drop-ready");
        scope.append(ghost);
        element.setPointerCapture(event.pointerId);
      }
      event.preventDefault();
      ghost.style.left = event.clientX + 10 + "px";
      ghost.style.top = event.clientY + 10 + "px";
      const next = targetAt(event);
      if (hover?.element !== next?.element) hover?.element.classList.remove("drop-active");
      next?.element.classList.add("drop-active");
      hover = next;
    }
    function stop(event) {
      if (event.pointerId !== start.pointerId) return;
      listeners.abort();
      if (!ghost) return;
      ghost.remove();
      for (const node of ready) node.classList.remove("drop-ready");
      hover?.element.classList.remove("drop-active");
      const suppress = click => { click.preventDefault(); click.stopImmediatePropagation(); };
      element.addEventListener("click", suppress, { capture: true, once: true });
      setTimeout(() => element.removeEventListener("click", suppress, true), 80);
      if (event.type !== "pointerup") return;
      const match = targetAt(event);
      if (match) match.target.receive(match.accepted, payload.get(match.accepted), event);
    }
    window.addEventListener("pointermove", move, controls);
    window.addEventListener("pointerup", stop, controls);
    window.addEventListener("pointercancel", stop, controls);
  });
}

export function dropTarget(element, types, receive) {
  element.dataset.dropTarget = "";
  targets.set(element, { types, receive });
}

export function bindItemActions(element, code, actions, { drag = true } = {}) {
  if (!code || !actions) return element;
  if (drag) dragSource(element, "item", code);
  element.setAttribute("aria-haspopup", "menu");
  const menu = event => {
    event.preventDefault();
    event.stopPropagation();
    showContextMenu(element, actions(code), event.type === "contextmenu" && event.clientX
      ? [event.clientX, event.clientY] : undefined);
  };
  element.addEventListener("contextmenu", menu);
  element.addEventListener("keydown", event => {
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) menu(event);
  });
  return element;
}

export function slotArt(item, artwork) {
  return item ? artwork?.icon(item.code) ?? catalogIcon("Items") : null;
}

export function slotTooltip(anchor, name, lines) {
  // An empty title also suppresses native tooltips inherited from ancestors.
  anchor.title = "";
  let tip;
  function hide() { tip?.remove(); tip = undefined; }
  anchor.addEventListener("pointerenter", () => {
    hide();
    if (document.querySelector(".record-menu:popover-open")) return;
    tip = el("div", { className: "slot-tooltip", popover: "manual", "aria-hidden": "true" },
      el("strong", {}, name), lines.filter(Boolean).map(line => el("span", {}, line)));
    document.body.append(tip);
    tip.showPopover();
    const bounds = anchor.getBoundingClientRect();
    tip.style.left = Math.max(4, Math.min(bounds.left, window.innerWidth - tip.offsetWidth - 4)) + "px";
    const below = bounds.bottom + 5;
    tip.style.top = (below + tip.offsetHeight < window.innerHeight ? below : Math.max(4, bounds.top - tip.offsetHeight - 5)) + "px";
  });
  for (const event of ["pointerleave", "pointerdown", "focusout"]) anchor.addEventListener(event, hide);
  return hide;
}

export function itemPalette(items, artwork, choose, named = false) {
  let page = 0, picked;
  let clearTooltips = [];
  const take = 40;
  const find = el("input", { type: "search", placeholder: "Filter items", "aria-label": "Filter item palette" });
  const slots = el("div", { className: "workbench-slots", "aria-label": "Item palette" });
  const status = el("span", { className: "palette-status", role: "status" });
  const previous = button("‹", () => { page--; render(); }, "Previous items");
  const next = button("›", () => { page++; render(); }, "Next items");
  previous.setAttribute("aria-label", "Previous items");
  next.setAttribute("aria-label", "Next items");
  function render() {
    const query = find.value.toLowerCase().trim().split(/\s+/);
    const matches = items.filter(item => query.every(term => item.label.toLowerCase().includes(term)));
    const visible = matches.slice(page * take, (page + 1) * take);
    for (const hide of clearTooltips) hide();
    clearTooltips = [];
    slots.replaceChildren(...visible.map(item => {
      const slot = button("", () => {
        picked = item.code;
        for (const child of slots.children) child.setAttribute("aria-pressed", String(child.dataset.code === picked));
        choose(item);
      }, "", "item-slot");
      slot.dataset.code = item.code;
      slot.setAttribute("aria-label", item.label);
      slot.setAttribute("aria-pressed", String(item.code === picked));
      slot.append(slotArt(item, artwork), ...(named ? [el("span", {}, item.label)] : []));
      dragSource(slot, "item", item.code);
      clearTooltips.push(slotTooltip(slot, item.label, []));
      return slot;
    }));
    status.textContent = matches.length ? (page * take + 1) + "–" + (page * take + visible.length) + " / " + matches.length : "No items";
    previous.disabled = page === 0;
    next.disabled = (page + 1) * take >= matches.length;
  }
  find.oninput = () => { page = 0; render(); };
  find.onkeydown = event => {
    if (event.key === "Enter") { event.preventDefault(); slots.firstElementChild?.click(); }
  };
  render();
  return el("aside", { className: "workbench-palette" + (named ? " named" : "") }, el("h3", {}, named ? "Materials" : "Item Palette"), find, slots,
    el("div", { className: "workbench-pages" }, previous, status, next));
}
