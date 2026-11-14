import { el, button } from "./inspector.mjs";

export function showContextMenu(anchor, actions, point) {
  const menu = el("div", { className: "record-menu", popover: "auto", role: "menu", "aria-label": "Record actions" });
  const buttons = actions.map(({ label, run, key }) => {
    const entry = button("", () => {
      menu.hidePopover();
      if (anchor.isConnected) anchor.focus({ preventScroll: true });
      run();
    });
    entry.setAttribute("role", "menuitem");
    entry.append(el("span", {}, label));
    if (key) entry.append(el("kbd", {}, key));
    return entry;
  });
  if (!buttons.length) return;
  menu.append(...buttons);
  document.body.append(menu);
  menu.addEventListener("toggle", event => {
    if (event.newState === "closed") menu.remove();
  });
  menu.onkeydown = event => {
    const index = buttons.indexOf(document.activeElement);
    const next = ({ ArrowDown: (index + 1) % buttons.length, ArrowUp: (index + buttons.length - 1) % buttons.length,
      Home: 0, End: buttons.length - 1 })[event.key];
    if (next !== undefined) { event.preventDefault(); buttons[next].focus(); }
    if (event.key === "Escape") {
      event.preventDefault();
      menu.hidePopover();
      if (anchor.isConnected) anchor.focus({ preventScroll: true });
    }
  };
  menu.showPopover();
  const origin = anchor.getBoundingClientRect();
  const x = point?.[0] ?? origin.left, y = point?.[1] ?? origin.bottom + 3;
  menu.style.left = Math.max(4, Math.min(x, window.innerWidth - menu.offsetWidth - 4)) + "px";
  menu.style.top = Math.max(4, Math.min(y, window.innerHeight - menu.offsetHeight - 4)) + "px";
  buttons[0].focus();
}

/** Clipboard write that also works where the async API is missing or denied; always returns a promise. */
export async function copyText(text) {
  try {
    if (navigator.clipboard?.writeText) return await navigator.clipboard.writeText(text);
  } catch { /* fall through to the legacy path */ }
  const area = el("textarea", { value: text, "aria-hidden": "true" });
  area.style.cssText = "position:fixed;opacity:0;pointer-events:none";
  document.body.append(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  if (!ok) throw new Error("Clipboard is unavailable in this browser context.");
}
