// Shared shell and command palette for Wand Lab, kept outside wand-lab-ui.mjs
// so the page's own module stays owned by its authors.
import { installPalette, registerCommands } from "./command-palette.mjs";
import { mountShell } from "./forge-shell.mjs";

const $ = id => document.getElementById(id);
const click = selector => () => document.querySelector(selector)?.click();
mountShell({ current: "/wand-lab" });
installPalette();
registerCommands(() => [
  ...[...document.querySelectorAll("nav.tabs [data-tab]")].map(tab => ({
    id: "wand:tab:" + tab.dataset.tab, label: "Show " + tab.textContent.trim(), group: "Tab", run: () => tab.click() })),
  { id: "wand:search", label: "Filter spells", group: "Action", keys: "/", run: () => { $("search")?.focus(); $("search")?.select(); } },
  ...[["#pin", "Pin current build as A"], ["#fill", "Fill empty slots"], ["#clear", "Clear deck"], ["#share", "Copy build code"],
    ["#import", "Paste build code"], ["#equip", "Equip in playtest"], ["#reroll", "Random found-wand seed"], ["#help", "Keyboard shortcuts"]]
    .filter(([selector]) => document.querySelector(selector))
    .map(([selector, label]) => ({ id: "wand:" + selector.slice(1), label, group: "Action", run: click(selector) })),
]);
