const MIN = 0.35, MAX = 2;
const clamp = value => Math.min(MAX, Math.max(MIN, Math.round(value * 100) / 100));

/**
 * Pan and zoom for a scrolling node surface. The surface is the scroller's first child and may be
 * replaced on every render, so zoom lives here and `apply()` re-stamps it after each render.
 * Pan: drag empty space, middle-drag, or Space+drag. Zoom: Ctrl/Cmd+wheel or `zoomBy`.
 * `onChange` runs after any zoom change so the wire layer can redraw.
 */
export function canvasView(scroll, { onChange = () => {}, nodeSelector = "[data-wire-target]" } = {}) {
  let zoom = 1, spaceDown = false;
  const lifetime = new AbortController(), signal = lifetime.signal;
  const surface = () => scroll.firstElementChild;

  function apply() {
    const node = surface();
    if (!node) return;
    node.style.transformOrigin = "0 0";
    node.style.transform = zoom === 1 ? "" : "scale(" + zoom + ")";
  }
  /** Zoom while keeping the layout point under (cx, cy) in scroller pixels fixed. */
  function setZoom(next, cx = scroll.clientWidth / 2, cy = scroll.clientHeight / 2) {
    const target = clamp(next);
    if (target === zoom) return;
    const x = (scroll.scrollLeft + cx) / zoom, y = (scroll.scrollTop + cy) / zoom;
    zoom = target;
    apply();
    scroll.scrollLeft = x * zoom - cx;
    scroll.scrollTop = y * zoom - cy;
    onChange(zoom);
  }
  const zoomBy = factor => setZoom(zoom * factor);
  function fit() {
    const cards = [...(surface()?.querySelectorAll(nodeSelector) ?? [])].filter(node => node.style.left);
    if (!cards.length) return;
    let right = 0, bottom = 0, left = Infinity, top = Infinity;
    for (const node of cards) {
      const x = parseFloat(node.style.left), y = parseFloat(node.style.top);
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x + node.offsetWidth);
      bottom = Math.max(bottom, y + node.offsetHeight);
    }
    const next = clamp(Math.min(1, scroll.clientWidth / (right - left + 48), scroll.clientHeight / (bottom - top + 48)));
    const changed = next !== zoom;
    zoom = next;
    apply();
    scroll.scrollLeft = Math.max(0, (left - 24) * zoom);
    scroll.scrollTop = Math.max(0, (top - 24) * zoom);
    if (changed) onChange(zoom);
  }
  const reset = () => { setZoom(1); };

  scroll.addEventListener("wheel", event => {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const bounds = scroll.getBoundingClientRect();
    setZoom(zoom * (event.deltaY < 0 ? 1.1 : 1 / 1.1), event.clientX - bounds.left, event.clientY - bounds.top);
  }, { passive: false });

  window.addEventListener("keydown", event => { if (event.code === "Space" && scroll.contains(document.activeElement)) spaceDown = true; }, { signal });
  window.addEventListener("keyup", event => { if (event.code === "Space") spaceDown = false; }, { signal });

  scroll.addEventListener("pointerdown", event => {
    const empty = event.target === scroll || event.target === surface();
    const middle = event.button === 1;
    if (!(middle || (event.button === 0 && (empty || spaceDown)))) return;
    event.preventDefault();
    const start = { x: event.clientX, y: event.clientY, left: scroll.scrollLeft, top: scroll.scrollTop };
    scroll.setPointerCapture(event.pointerId);
    scroll.classList.add("panning");
    const move = move => {
      scroll.scrollLeft = start.left - (move.clientX - start.x);
      scroll.scrollTop = start.top - (move.clientY - start.y);
    };
    const end = () => {
      scroll.classList.remove("panning");
      scroll.removeEventListener("pointermove", move);
      scroll.removeEventListener("pointerup", end);
      scroll.removeEventListener("pointercancel", end);
    };
    scroll.addEventListener("pointermove", move);
    scroll.addEventListener("pointerup", end);
    scroll.addEventListener("pointercancel", end);
  });

  return { destroy: () => lifetime.abort(), apply, fit, reset, zoomBy, setZoom, zoom: () => zoom, isEmptyTarget: target => target === scroll || target === surface() };
}
