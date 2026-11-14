// Wand Lab page. Every stat comes from /api/wand-lab/* (the native evaluator);
// this file edits the build and draws what the evaluator returns. Derived
// views (time to kill, cumulative damage, flight paths) are labelled where
// they extrapolate from native cast blocks.
const token = location.hash.slice(1);
const $ = id => document.getElementById(id);
const el = (tag, props = {}, ...kids) => {
  const { style, ...rest } = props;
  const n = Object.assign(document.createElement(tag), rest);
  if (style) for (const [k, v] of Object.entries(style)) n.style.setProperty(k, v);
  for (const k of kids.flat(Infinity)) if (k != null && k !== false) n.append(k);
  return n;
};
const STORE = "axiom.wandLab.v2";
const fmt = (v, d = 1) => v == null || !Number.isFinite(Number(v)) ? "—" : Number(v).toFixed(d);
// Spell colours are linear RGB.
const rgb = c => c ? `rgb(${c.map(v => Math.round(Math.min(1, Math.max(0, v)) ** (1 / 2.2) * 255)).join(",")})` : "#556";

async function api(route, body) {
  const response = await fetch(route, body === undefined
    ? { headers: { "X-Axiom-Session": token } }
    : { method: "POST", headers: { "X-Axiom-Session": token, "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || response.statusText);
  return data;
}

const state = {
  catalog: null, byId: new Map(), result: null, shipped: null, pinned: null,
  filter: "All", query: "", sort: "id", castableOnly: true, tab: "perf",
  build: { tier: 3, mode: 0, rank: 0, seed: 0, health: 1, foe_visible: true, slots: [] },
  locked: [], compare: [], enemies: [], draft: { active: false, changes: [], fields: [] },
  survey: null, surveySort: { key: "vs_reference", dir: -1 },
  undo: [], redo: [],
};
const remembered = (() => { try { return JSON.parse(localStorage.getItem(STORE) ?? "{}"); } catch { return {}; } })();
Object.assign(state.build, remembered.build ?? {});
state.locked = remembered.locked ?? [];
state.compare = remembered.compare ?? [];
const persist = () => { try { localStorage.setItem(STORE, JSON.stringify({ build: state.build, locked: state.locked, compare: state.compare })); } catch { /* private window */ } };

const status = (text, kind = "") => { const s = $("status"); s.textContent = text; s.className = "status " + kind; };
const capacity = () => state.catalog?.capacity[state.build.tier - 1] ?? 8;
const EMPTY = () => state.catalog?.empty ?? 8;
const spell = id => state.byId.get(id);
const draftIds = () => new Set(state.draft.changes.map(c => c.id));

function normaliseSlots() {
  const cap = capacity();
  const slots = state.build.slots.slice(0, cap);
  while (slots.length < cap) slots.push(EMPTY());
  state.build.slots = slots;
  state.locked = state.locked.filter(i => i < cap);
}

// ---------- history ----------
const snapshot = () => JSON.stringify(state.build);
let lastSnapshot = null;
function remember() {
  const now = snapshot();
  if (lastSnapshot && lastSnapshot !== now) { state.undo.push(lastSnapshot); if (state.undo.length > 100) state.undo.shift(); state.redo = []; }
  lastSnapshot = now;
  $("undo").disabled = !state.undo.length; $("redo").disabled = !state.redo.length;
}
async function travel(from, to) {
  if (!from.length) return;
  to.push(snapshot());
  const tier = state.build.tier;
  state.build = JSON.parse(from.pop());
  lastSnapshot = snapshot();
  syncControls();
  if (tier !== state.build.tier) await loadCatalog();
  normaliseSlots();
  $("undo").disabled = !state.undo.length; $("redo").disabled = !state.redo.length;
  changed(false);
}

// ---------- palette ----------
function renderClasses() {
  const classes = ["All", ...new Set(state.catalog.spells.filter(s => s.palette !== "Hidden").map(s => s.class))];
  $("classes").replaceChildren(...classes.map(c => el("button", {
    textContent: c, ariaPressed: String(state.filter === c),
    onclick: () => { state.filter = c; renderClasses(); renderPalette(); },
  })));
}

function visibleSpells() {
  const q = state.query.toLowerCase();
  const rows = state.catalog.spells.filter(s => s.palette !== "Hidden"
    && (!state.castableOnly || s.available)
    && (state.filter === "All" || s.class === state.filter)
    && (!q || s.name.toLowerCase().includes(q) || s.tag.toLowerCase().includes(q) || (s.hint ?? "").toLowerCase().includes(q)));
  const key = { id: s => s.id, name: s => s.name, cost: s => s.cost, damage: s => -s.damage, dpm: s => -(s.damage / Math.max(1, s.cost)) }[state.sort];
  return rows.sort((a, b) => { const x = key(a), y = key(b); return typeof x === "string" ? x.localeCompare(y) : x - y; });
}

function renderPalette() {
  const drafted = draftIds();
  $("cards").replaceChildren(...visibleSpells().map(s => el("div", { className: "card" + (s.available ? "" : " locked"), draggable: true, tabIndex: 0,
      title: `${s.name} · ${s.class} · ${s.tag}\n${s.hint ?? ""}\nMana ${s.cost} · damage ${s.damage} · speed ${s.speed} · life ${fmt(s.life, 2)} s\nCooldown ${s.cooldown} s · delay ${s.cast_delay >= 0 ? "+" : ""}${s.cast_delay} s · recharge ${s.recharge >= 0 ? "+" : ""}${s.recharge} s\nClick/Enter: add · double-click or E: edit stats`,
      ondragstart: e => e.dataTransfer.setData("text/plain", "spell:" + s.id),
      onclick: () => place(s.id), ondblclick: () => openEditor(s.id),
      onkeydown: e => { if (e.key === "Enter") place(s.id); if (e.key === "e") openEditor(s.id); },
      style: { "--card": rgb(s.color) } },
    el("span", { className: "dot" }),
    el("span", {}, el("div", { textContent: s.name + (drafted.has(s.id) ? " ✎" : "") }), el("div", { className: "muted small", textContent: s.class })),
    el("span", { className: "stats", innerHTML: `${fmt(s.cost, 0)} mana<br>${s.class === "Projectile" ? fmt(s.damage, 0) + " dmg" : s.tag.toLowerCase()}` }))));
}

function place(id) {
  const i = state.build.slots.findIndex((s, k) => s === EMPTY() && !state.locked.includes(k));
  if (i < 0) return status(`Deck is full (${capacity()} slots). Clear a slot first.`, "error");
  state.build.slots[i] = id;
  changed();
}

// ---------- deck ----------
function renderDeck() {
  const usable = new Map((state.result?.slots ?? []).map(s => [s.slot, s.usable]));
  const drafted = draftIds();
  $("deck").replaceChildren(...state.build.slots.map((id, i) => {
    const s = spell(id);
    const filled = id !== EMPTY() && s;
    const locked = state.locked.includes(i);
    const li = el("li", { className: ["slot", filled && "filled", filled && usable.get(i) === false && "unusable", locked && "locked", filled && drafted.has(id) && "draft-spell"].filter(Boolean).join(" "),
      draggable: !!filled, tabIndex: 0, style: filled ? { "--card": rgb(s.color) } : {},
      title: filled ? `${s.name} — ${s.class}\n${s.hint ?? ""}\nRight-click or Delete clears · E edits · L locks` : "Empty slot. Drop a spell here.",
      ondragstart: e => e.dataTransfer.setData("text/plain", "slot:" + i),
      ondragover: e => { e.preventDefault(); li.classList.add("over"); },
      ondragleave: () => li.classList.remove("over"),
      ondrop: e => {
        e.preventDefault(); li.classList.remove("over");
        const [kind, value] = e.dataTransfer.getData("text/plain").split(":");
        const n = Number(value);
        if (kind === "spell") state.build.slots[i] = n;
        if (kind === "slot" && n !== i) [state.build.slots[i], state.build.slots[n]] = [state.build.slots[n], state.build.slots[i]];
        changed();
      },
      oncontextmenu: e => { e.preventDefault(); clearSlot(i); },
      onkeydown: e => {
        if (e.target !== li) return;
        if (e.key === "Delete" || e.key === "Backspace") clearSlot(i);
        if (e.key === "ArrowLeft" && i > 0) swap(i, i - 1);
        if (e.key === "ArrowRight" && i < state.build.slots.length - 1) swap(i, i + 1);
        if (e.key === "e" && filled) openEditor(id);
        if (e.key === "l") toggleLock(i);
      } },
      el("button", { className: "lock", textContent: "🔒", ariaPressed: String(locked), title: "Lock for the optimiser (L)", onclick: e => { e.stopPropagation(); toggleLock(i); } }),
      filled ? [el("span", { className: "glyph" }), el("span", { className: "name", textContent: s.name }),
        el("span", { className: "meta", textContent: `${s.class} · ${fmt(s.cost, 0)} mana` }),
        el("button", { className: "edit", textContent: "✎", title: "Edit stats (E)", onclick: e => { e.stopPropagation(); openEditor(id); } })]
        : el("span", { className: "meta", textContent: "empty" }));
    return li;
  }));
}
const clearSlot = i => { state.build.slots[i] = EMPTY(); changed(); };
function toggleLock(i) {
  state.locked = state.locked.includes(i) ? state.locked.filter(k => k !== i) : [...state.locked, i];
  persist(); renderDeck();
}
function swap(a, b) {
  [state.build.slots[a], state.build.slots[b]] = [state.build.slots[b], state.build.slots[a]];
  changed();
  requestAnimationFrame(() => $("deck").children[b]?.focus());
}

function renderCastStrip() {
  const r = state.result;
  if (!r?.blocks.length) return $("cast-strip").replaceChildren(el("span", { textContent: "Add a projectile to see the cast order." }));
  const parts = [];
  r.blocks.forEach((b, i) => {
    if (i) parts.push(el("span", { className: "arrow", textContent: "→" }));
    const mods = b.modifier_slots.map(k => spell(state.build.slots[k])?.name).filter(Boolean);
    parts.push(el("span", { className: "step", title: `t ${fmt(b.start_s, 2)} s · ${fmt(b.mana)} mana · ${fmt(b.damage, 0)} dmg` },
      b.actions.slice(0, 4).map(a => el("i", { style: { "--card": rgb(spell(a.spell)?.color) } })),
      `${mods.length ? mods.join("+") + " › " : ""}${b.actions.map(a => a.name).join(" + ")}`));
  });
  if (r.summary.wrapped) parts.push(el("span", { className: "wrap", textContent: `↻ recharge ${fmt(r.blocks.at(-1).recharge, 2)} s` }));
  if (r.summary.loops) parts.push(el("span", { className: "wrap", textContent: "⚠ never recharges: the evaluator kept cycling for 64 casts" }));
  if (r.summary.health_per_s > 0) parts.push(el("span", { className: "wrap", textContent: `♥ pays ${fmt(r.summary.health_per_s)} health/s` }));
  $("cast-strip").replaceChildren(...parts);
}

// ---------- evaluation ----------
let timer, generation = 0;
function changed(record = true) {
  if (record) remember();
  persist();
  renderDeck();
  clearTimeout(timer);
  timer = setTimeout(evaluate, 90);
}

async function evaluate() {
  const mine = ++generation;
  try {
    const { current, shipped } = await api("/api/wand-lab/evaluate", state.build);
    if (mine !== generation) return;
    state.result = current; state.shipped = shipped && !shipped.error ? shipped : null;
    status(`Native evaluator${current.draft ? " · DRAFT spell table" : ""} · ${current.chassis.name} · ${current.chassis.chassis}`, current.draft ? "" : "ok");
    renderDeck(); renderCastStrip(); renderResults(); animate();
  } catch (error) {
    if (mine === generation) status(error.message, "error");
  }
}

function tile(label, value, { base, better = "up", unit = "", digits = 1, cls = "", shipped } = {}) {
  const delta = (ref, tag) => {
    if (ref == null || typeof value !== "number") return null;
    const d = value - ref;
    if (Math.abs(d) < 1e-6) return null;
    const good = better === "up" ? d > 0 : d < 0;
    return el("div", { className: "delta " + (good ? "up" : "down"), textContent: `${d > 0 ? "+" : ""}${d.toFixed(digits)}${unit} vs ${tag}` });
  };
  return el("div", { className: "tile " + cls },
    el("div", { className: "label", textContent: label }),
    el("div", { className: "value", textContent: typeof value === "number" ? value.toFixed(digits) + unit : value }),
    delta(base, "A"), delta(shipped, "shipped"));
}

function renderResults() {
  const r = state.result; if (!r || state.tab !== "perf") return;
  const s = r.summary, c = r.chassis, a = state.pinned, sh = state.shipped?.summary;
  $("tiles").replaceChildren(...[
    tile("Damage / s", s.dps, { base: a?.dps, shipped: sh?.dps }),
    tile("Sustained (30 s)", s.dps_30s, { base: a?.dps_30s, shipped: sh?.dps_30s }),
    tile("Damage / cycle", s.damage_per_cycle, { base: a?.damage_per_cycle, digits: 0, shipped: sh?.damage_per_cycle }),
    tile("Mana / s", s.mana_per_s, { base: a?.mana_per_s, better: "down", cls: s.sustainable ? "good" : "bad", shipped: sh?.mana_per_s }),
    tile("Mana charge", c.mana_charge, { unit: "/s" }),
    tile("Sustain", s.sustainable ? "Endless" : `${fmt(s.seconds_to_empty)} s`, { cls: s.sustainable ? "good" : "bad" }),
    tile("Cycle", s.cycle_s, { base: a?.cycle_s, better: "down", unit: " s", digits: 2, shipped: sh?.cycle_s }),
    tile("Damage / mana", s.damage_per_mana, { base: a?.damage_per_mana, digits: 2, shipped: sh?.damage_per_mana }),
    tile("Casts · bolts", `${s.casts} · ${s.bolts}`),
    tile("Mana pool", c.mana_max, { digits: 0 }),
    s.health_per_s > 0 ? tile("Health / s", s.health_per_s, { better: "down", cls: "bad", shipped: sh?.health_per_s }) : null,
  ].filter(Boolean));
  drawTimeline(); drawMana(); drawFlight(); renderKills();
  $("blocks").replaceChildren(...r.blocks.map((b, i) => el("li", {},
    el("div", { className: "head" },
      el("strong", { textContent: `#${i + 1}` }),
      el("span", { textContent: `t ${fmt(b.start_s, 2)} s` }),
      el("span", { textContent: `slots ${Math.min(b.first_slot, ...b.modifier_slots) + 1}–${b.last_slot + 1}` }),
      el("span", { textContent: b.health ? `${fmt(b.health)} health` : `${fmt(b.mana)} mana` }),
      el("span", { textContent: `delay ${fmt(b.cast_delay, 2)} s` }),
      b.recharge > 0 ? el("span", { textContent: `recharge ${fmt(b.recharge, 2)} s` }) : null,
      el("span", { textContent: `${fmt(b.damage, 0)} dmg` }),
      b.spread_deg ? el("span", { textContent: `spread ${fmt(b.spread_deg)}°` }) : null),
    b.actions.map(action))));
  $("scope").textContent = r.scope;
}

function action(a) {
  const tags = [];
  if (a.mods.damage_mul !== 1) tags.push(el("span", { className: "tag", textContent: `×${fmt(a.mods.damage_mul, 2)} dmg` }));
  if (a.mods.speed_mul !== 1) tags.push(el("span", { className: "tag", textContent: `×${fmt(a.mods.speed_mul, 2)} speed` }));
  if (a.mods.life_mul !== 1) tags.push(el("span", { className: "tag", textContent: `×${fmt(a.mods.life_mul, 2)} life` }));
  if (a.mods.element) tags.push(el("span", { className: "tag el", textContent: a.mods.element }));
  for (const [k, label] of [["bounces", "bounce"], ["pierce", "pierce"], ["phase", "phase"]]) if (a.mods[k]) tags.push(el("span", { className: "tag", textContent: `${a.mods[k]} ${label}` }));
  for (const f of a.mods.flags) tags.push(el("span", { className: "tag", textContent: f.replace("_", " ") }));
  return el("div", { className: "action" }, el("span", { className: "spell", textContent: a.name, style: { "--card": rgb(spell(a.spell)?.color) } }), ...tags,
    a.payload ? el("span", { className: "tag", textContent: `→ carries ${a.payload.name}` }) : null,
    a.follow ? el("span", { className: "tag", textContent: `then ${a.follow.name}` }) : null,
    a.flight ? el("span", { className: "muted small", textContent: `${fmt(a.flight.speed, 0)} m/s · ${fmt(a.flight.speed * a.flight.life, 0)} m reach` }) : null,
    a.hit_damage ? el("span", { className: "muted small", textContent: `${fmt(a.hit_damage, 0)} dmg` }) : null);
}

// ---------- held-fire model (extrapolates native cast blocks) ----------
// Replays the cycle with a mana pool, waiting for charge when short. Returns
// [t, cumulative damage, pool] samples and when `target` damage has landed.
function fire(result, seconds, target = Infinity) {
  const c = result.chassis, blocks = result.blocks;
  const points = [[0, 0, c.mana_max]];
  let pool = c.mana_max, t = 0, dealt = 0, i = 0, killedAt = null, casts = 0, spent = 0;
  while (blocks.length && t < seconds && i < 20000) {
    const b = blocks[i % blocks.length];
    if (pool < b.mana) {
      if (c.mana_charge <= 0) break;
      t += (b.mana - pool) / c.mana_charge; pool = b.mana;
      points.push([t, dealt, pool]);
    }
    pool -= b.mana; spent += b.mana; dealt += b.damage; casts++;
    points.push([t, dealt, pool]);
    if (killedAt == null && dealt >= target) { killedAt = { t, casts, mana: spent }; break; }
    const step = b.cast_delay + b.recharge;
    t += step; pool = Math.min(c.mana_max, pool + c.mana_charge * step);
    points.push([t, dealt, pool]);
    i++;
  }
  return { points, killedAt };
}

function renderKills() {
  const r = state.result, q = $("enemy-filter").value.toLowerCase(), count = Math.max(1, Number($("targets").value) || 1);
  if (!state.enemies.length) return $("kills").replaceChildren(el("p", { className: "muted small", textContent: "No bestiary rows found." }));
  if (!r.summary.damage_per_cycle) return $("kills").replaceChildren(el("p", { className: "muted small", textContent: "This deck deals no direct-hit damage." }));
  const rows = state.enemies.filter(e => !q || e.name.toLowerCase().includes(q) || e.role.toLowerCase().includes(q))
    .map(e => ({ e, k: fire(r, 600, e.health * count).killedAt }));
  const worst = Math.max(1, ...rows.map(x => x.k?.t ?? 0));
  $("kills").replaceChildren(...rows.map(({ e, k }) => {
    const stalls = k && k.t > (r.summary.seconds_to_empty ?? Infinity);
    return el("div", { className: "kill" + (!k ? " stall" : stalls ? " slow" : ""), title: `${e.role} · ${e.health} hp each${count > 1 ? ` × ${count}` : ""}` },
      el("strong", { textContent: e.name }),
      el("span", { className: "muted", textContent: k ? `${fmt(k.t, 2)} s · ${k.casts} casts · ${fmt(k.mana, 0)} mana${stalls ? " · pool ran dry" : ""}` : "over 10 min" }),
      el("div", { className: "bar" }, el("span", { style: { width: `${k ? Math.max(3, (k.t / worst) * 100) : 100}%` } })));
  }));
}

// ---------- charts ----------
function canvas(id) {
  const c = $(id), ratio = devicePixelRatio || 1, w = c.clientWidth, h = c.clientHeight;
  c.width = w * ratio; c.height = h * ratio;
  const g = c.getContext("2d"); g.scale(ratio, ratio); g.font = "11px Segoe UI";
  return { g, w, h };
}

function drawTimeline() {
  const { g, w, h } = canvas("timeline"), r = state.result;
  const total = Math.max(r.summary.cycle_s, 0.05), pad = 8, x = t => pad + (t / total) * (w - 2 * pad);
  const peak = Math.max(1, ...r.blocks.map(b => b.damage));
  g.strokeStyle = "#303e51"; g.beginPath(); g.moveTo(pad, h - 18); g.lineTo(w - pad, h - 18); g.stroke();
  for (const b of r.blocks) {
    const x0 = x(b.start_s), x1 = Math.max(x0 + 3, x(b.start_s + b.cast_delay));
    const bh = Math.max(4, (b.damage / peak) * (h - 40));
    // One colour band per action, so multicasts read as several bolts.
    let y = h - 18;
    for (const a of b.actions) {
      const part = b.damage ? (a.hit_damage / b.damage) * bh : bh / b.actions.length;
      g.fillStyle = rgb(spell(a.spell)?.color); g.fillRect(x0, y - part, x1 - x0 - 1, part); y -= part;
    }
    if (b.recharge > 0) {
      const r1 = x(b.start_s + b.cast_delay + b.recharge);
      g.save(); g.beginPath(); g.rect(x1, h - 40, r1 - x1, 22); g.clip();
      g.strokeStyle = "#9cacc1"; for (let i = x1 - 30; i < r1; i += 6) { g.beginPath(); g.moveTo(i, h - 18); g.lineTo(i + 22, h - 40); g.stroke(); }
      g.restore();
    }
  }
  g.fillStyle = "#9cacc1";
  g.fillText("0 s", pad, h - 4); const end = `${total.toFixed(2)} s`; g.fillText(end, w - pad - g.measureText(end).width, h - 4);
}

function drawMana() {
  const { g, w, h } = canvas("mana"), r = state.result, c = r.chassis;
  const span = 15, pad = 8, x = t => pad + (t / span) * (w - 2 * pad), y = m => h - 16 - (m / c.mana_max) * (h - 28);
  const { points } = fire(r, span);
  g.strokeStyle = "#303e51"; g.beginPath(); g.moveTo(pad, y(0)); g.lineTo(w - pad, y(0)); g.stroke();
  g.strokeStyle = r.summary.sustainable ? "#98d8b1" : "#f8c67b"; g.lineWidth = 2; g.beginPath();
  points.filter(p => p[0] <= span).forEach(([pt, , m], k) => k ? g.lineTo(x(pt), y(m)) : g.moveTo(x(pt), y(m)));
  g.stroke(); g.lineWidth = 1;
  g.fillStyle = "#9cacc1"; g.fillText(`${c.mana_max.toFixed(0)}`, pad + 2, y(c.mana_max) + 11); g.fillText("15 s", w - pad - 24, h - 3);
}

// Side view of each distinct bolt from its native speed, life, gravity,
// bounces and path mode. Illustrative, not a physics replay.
function drawFlight() {
  const { g, w, h } = canvas("flight"), r = state.result;
  const shots = [], seen = new Set();
  for (const b of r.blocks) for (const a of b.actions) {
    if (!a.flight || !a.hit_damage) continue;
    const key = a.spell + JSON.stringify(a.flight);
    if (!seen.has(key) && shots.length < 6) { seen.add(key); shots.push(a); }
  }
  // Trace in metres first, then fit both axes so arcs and wobble stay visible.
  const traces = shots.map(a => {
    const f = a.flight, pts = [[0, 1.6]];
    let px = 0, py = 1.6, vyv = 0, bounces = f.bounces, t = 0;
    const dt = Math.max(0.005, f.life / 300);
    while (t < f.life) {
      t += dt;
      vyv -= 14 * f.gravity * dt;
      px += f.speed * dt; py += vyv * dt;
      let wobble = 0;
      if (f.path === 1) wobble = Math.sin(t * 2.2) * 1.5;
      if (f.path === 2) wobble = Math.sin(t * 18) * 0.6;
      if (f.path === 3 || f.orbit) wobble = Math.sin(t * 6) * 2;
      if (py < 0) { if (bounces > 0) { py = -py; vyv = -vyv * 0.7; bounces--; } else { py = 0; pts.push([px, 0]); break; } }
      pts.push([px, Math.max(0, py + wobble)]);
    }
    return { a, pts };
  });
  const reach = Math.max(10, ...traces.flatMap(t => t.pts.map(p => p[0])));
  const top = Math.max(4, ...traces.flatMap(t => t.pts.map(p => p[1]))) * 1.15;
  const pad = 10, ground = h - 14, X = m => pad + (m / reach) * (w - 2 * pad), Y = m => ground - (m / top) * (ground - 8);
  g.strokeStyle = "#303e51"; g.beginPath(); g.moveTo(0, ground); g.lineTo(w, ground); g.stroke();
  for (const { a, pts } of traces) {
    g.strokeStyle = rgb(spell(a.spell)?.color); g.lineWidth = 2; g.beginPath();
    pts.forEach(([x, y], i) => i ? g.lineTo(X(x), Y(y)) : g.moveTo(X(x), Y(y)));
    g.stroke(); g.lineWidth = 1;
  }
  g.fillStyle = "#9cacc1"; g.fillText(`${fmt(reach, 0)} m`, w - pad - 34, h - 2); g.fillText(`${fmt(top, 0)} m high`, pad, 12);
  $("flight-legend").textContent = traces.map(({ a }) => `${a.name} ${fmt(a.flight.speed, 0)} m/s, ${fmt(a.flight.speed * a.flight.life, 0)} m`).join(" · ") || "no damaging bolts";
}

// Replays one deck cycle on the slot cards so the cast order is visible.
let animation;
function animate() {
  clearTimeout(animation);
  const blocks = state.result.blocks, cards = [...$("deck").children];
  let i = 0;
  const step = () => {
    cards.forEach(c => c.classList.remove("firing"));
    if (i >= blocks.length) return;
    const b = blocks[i++];
    const used = new Set([...b.modifier_slots, ...b.actions.flatMap(a => [a.slot, a.payload?.slot].filter(v => v != null))]);
    for (const s of used) cards[s]?.classList.add("firing");
    animation = setTimeout(step, Math.min(900, Math.max(220, (b.cast_delay + b.recharge) * 1000)));
  };
  step();
}

// ---------- compare ----------
const PALETTE = ["#85d7e3", "#f8c67b", "#ff9c9c", "#98d8b1", "#c9a0ff", "#ffd36e"];
async function renderCompare() {
  if (!state.compare.length) {
    $("compare-table").replaceChildren(el("p", { className: "muted small", textContent: "Nothing to compare yet." }));
    const { g, w, h } = canvas("compare-chart"); g.fillStyle = "#9cacc1"; g.fillText("Add builds to compare", w / 2 - 50, h / 2);
    return;
  }
  const scored = await Promise.all(state.compare.map(async entry => {
    try { return { ...entry, result: (await api("/api/wand-lab/evaluate", entry.build)).current }; }
    catch (error) { return { ...entry, error: error.message }; }
  }));
  const ok = scored.filter(s => s.result);
  const { g, w, h } = canvas("compare-chart"), span = 30, pad = 34;
  const curves = scored.map(s => s.result ? fire(s.result, span).points : []);
  const top = Math.max(1, ...curves.flatMap(p => p.map(q => q[1])));
  g.strokeStyle = "#303e51"; g.beginPath(); g.moveTo(pad, h - 16); g.lineTo(w - 8, h - 16); g.stroke();
  curves.forEach((pts, k) => {
    g.strokeStyle = PALETTE[k % PALETTE.length]; g.lineWidth = 2; g.beginPath();
    pts.filter(p => p[0] <= span).forEach(([t, d], i) => { const X = pad + (t / span) * (w - pad - 8), Y = h - 16 - (d / top) * (h - 30); i ? g.lineTo(X, Y) : g.moveTo(X, Y); });
    g.stroke(); g.lineWidth = 1;
  });
  g.fillStyle = "#9cacc1"; g.fillText(fmt(top, 0), 2, 12); g.fillText("30 s", w - 30, h - 2);
  const metrics = [["dps", "Dmg/s", 1, 1], ["dps_30s", "30 s avg", 1, 1], ["damage_per_cycle", "Dmg/cycle", 0, 1], ["mana_per_s", "Mana/s", 1, -1],
    ["damage_per_mana", "Dmg/mana", 2, 1], ["cycle_s", "Cycle s", 2, -1], ["seconds_to_empty", "Dry in s", 1, 1]];
  const value = (s, k) => s.result ? (s.result.summary[k] ?? (k === "seconds_to_empty" ? Infinity : null)) : null;
  const best = Object.fromEntries(metrics.map(([k, , , dir]) => {
    const vals = ok.map(s => value(s, k)).filter(v => v != null);
    return [k, dir > 0 ? Math.max(...vals) : Math.min(...vals)];
  }));
  $("compare-table").replaceChildren(el("table", { className: "grid" },
    el("tr", {}, el("th", { textContent: "Build" }), metrics.map(([, label]) => el("th", { textContent: label })), el("th", {})),
    scored.map((s, k) => el("tr", {},
      el("td", {}, el("span", { className: "swatch", style: { "--card": PALETTE[k % PALETTE.length] } }), s.name, s.error ? el("span", { className: "muted small", textContent: " — " + s.error }) : null),
      metrics.map(([key, , d]) => {
        const v = value(s, key);
        return el("td", { className: v === best[key] && ok.length > 1 ? "best" : "", textContent: v === Infinity ? "endless" : fmt(v, d) });
      }),
      el("td", {}, el("button", { textContent: "Load", onclick: () => load(s.build) }),
        el("button", { textContent: "✕", onclick: () => { state.compare.splice(k, 1); persist(); renderCompare(); } }))))));
}

function addCompare(name, build) {
  if (state.compare.length >= 6) return status("Compare holds six builds.", "error");
  state.compare.push({ name, build: structuredClone(build) }); persist(); switchTab("compare");
}

// ---------- optimiser ----------
async function optimize() {
  const pool = visibleSpells().map(s => s.id);
  $("optimize").disabled = true; status("Searching on the native evaluator…");
  try {
    const out = await api("/api/wand-lab/optimize", { build: state.build, objective: $("objective").value, must_sustain: $("must-sustain").checked, allow_health_costs: $("allow-health").checked, allow_no_recharge: $("allow-norecharge").checked,
      locked: state.locked, pool, iterations: Number($("iterations").value), seed: Number($("opt-seed").value) || 0 });
    const side = (title, s, slots) => el("div", {}, el("strong", { textContent: title }), swatches(slots),
      el("div", { className: "small", textContent: `${fmt(s.dps)} dmg/s · ${fmt(s.dps_30s)} 30 s avg · ${fmt(s.mana_per_s)} mana/s · ${fmt(s.damage_per_mana, 2)} dmg/mana · ${s.sustainable ? "endless" : "runs dry"}${s.health_per_s > 0 ? ` · ${fmt(s.health_per_s)} health/s` : ""}${s.wrapped ? "" : " · never recharges"}` }));
    const before = structuredClone(state.build);
    $("opt-result").replaceChildren(
      el("div", { className: "opt-diff" }, side("Current", out.before, before.slots), side("Found", out.after, out.build.slots)),
      el("p", { className: "small", textContent: out.build.slots.map(id => spell(id)?.name ?? "empty").join(" · ") }),
      el("div", { className: "row" },
        el("button", { className: "primary", textContent: "Use this deck", onclick: () => load(out.build) }),
        el("button", { textContent: "Add both to compare", onclick: () => { addCompare("Before search", before); addCompare(`Found (${out.objective})`, out.build); } }),
        el("button", { textContent: "Search again (new seed)", onclick: () => { $("opt-seed").value = Number($("opt-seed").value) + 1; optimize(); } })),
      el("p", { className: "muted small", textContent: `${out.evaluated.toLocaleString()} decks evaluated. ${out.scope}` }));
    status("Search finished", "ok");
  } catch (error) { status(error.message, "error"); }
  finally { $("optimize").disabled = false; }
}

// ---------- balance survey ----------
function median(values) { const v = [...values].sort((a, b) => a - b); return v.length ? v[Math.floor(v.length / 2)] : 0; }
async function runSurvey() {
  $("survey").disabled = true; status("Surveying every spell…");
  try { state.survey = await api("/api/wand-lab/survey", { build: state.build, reference: Number($("reference").value) }); renderSurvey(); status("Survey done", "ok"); }
  catch (error) { status(error.message, "error"); }
  finally { $("survey").disabled = false; }
}
function renderSurvey() {
  const sv = state.survey; if (!sv) return;
  const cols = [["name", "Spell", 0], ["class", "Class", 0], ["cost", "Mana", 0], ["dps", "Dmg/s", 1], ["dps_30s", "30 s avg", 1],
    ["mana_per_s", "Mana/s", 1], ["damage_per_mana", "Dmg/mana", 2], ["vs_reference", "× ref", 2]];
  const { key, dir } = state.surveySort;
  const rows = [...sv.rows].sort((a, b) => typeof a[key] === "string" ? dir * a[key].localeCompare(b[key]) : dir * (a[key] - b[key]));
  const max = Object.fromEntries(cols.slice(2).map(([k]) => [k, Math.max(1e-6, ...sv.rows.map(r => r[k]))]));
  const med = {};
  for (const c of new Set(sv.rows.map(r => r.class))) med[c] = median(sv.rows.filter(r => r.class === c).map(r => r.damage_per_mana));
  const drafted = draftIds();
  $("survey-table").replaceChildren(
    el("p", { className: "muted small", textContent: `Reference: ${sv.reference.name} at ${fmt(sv.reference.dps)} dmg/s. ${sv.scope} Click a row to edit that spell.` }),
    el("table", { className: "grid" },
      el("tr", {}, cols.map(([k, label]) => el("th", { textContent: label + (k === key ? (dir > 0 ? " ▲" : " ▼") : ""),
        onclick: () => { state.surveySort = { key: k, dir: k === key ? -dir : -1 }; renderSurvey(); } }))),
      rows.map(r => {
        const off = med[r.class] ? r.damage_per_mana / med[r.class] : 1;
        const heat = off > 1.35 ? "hot" : off < 0.7 ? "cold" : "";
        return el("tr", { onclick: () => openEditor(r.id), style: { cursor: "pointer" }, title: `${fmt(off, 2)}× the ${r.class.toLowerCase()} median damage per mana` },
          el("td", {}, el("span", { className: "swatch", style: { "--card": rgb(r.color) } }), r.name + (drafted.has(r.id) ? " ✎" : ""),
            r.never_recharges ? el("span", { className: "tag el", title: "This card makes the reference deck skip recharge in the current evaluator", textContent: "no recharge" }) : null,
            r.health_per_s > 0 ? el("span", { className: "tag", textContent: `♥ ${fmt(r.health_per_s)}/s` }) : null),
          el("td", { textContent: r.class }), el("td", { textContent: fmt(r.cost, 0) }),
          cols.slice(3).map(([k, , d]) => el("td", { className: "cellbar " + (k === "damage_per_mana" ? heat : ""), style: { "--w": `${Math.max(0, r[k] / max[k]) * 100}%` } }, el("span", { textContent: fmt(r[k], d) }))));
      })));
}

async function balanceAI() {
  const goal = $("balance-goal").value.trim();
  if (!goal) return status("Describe the balance goal first.", "error");
  $("balance-ai").disabled = true; status("AI reading the balance sheet…");
  try {
    const out = await api("/api/wand-lab/balance/suggest", { goal, build: state.build });
    $("balance-ai-out").replaceChildren(el("ul", { className: "draft-list" }, out.changes.map(c => el("li", {},
      el("strong", { textContent: spell(c.spell)?.name ?? "#" + c.spell }), " ",
      Object.entries(c.values).map(([k, v]) => `${k} ${(k === "damage" ? spell(c.spell)?.base_damage : spell(c.spell)?.[k]) ?? "?"} → ${v}`).join(", "),
      c.why ? el("div", { className: "muted small", textContent: c.why }) : null,
      el("button", { textContent: "Add to draft", onclick: () => draftEdit(c.spell, c.values) })))),
      el("p", { className: "muted small", textContent: out.note }));
    status("Proposals ready; nothing is applied until you add them", "ok");
  } catch (error) { status(error.message, "error"); }
  finally { $("balance-ai").disabled = false; }
}

// ---------- spell editor (draft table) ----------
const RANGES = { cost: [0, 120, 0.5], cooldown: [0, 3, 0.01], cast_delay: [-0.5, 1.5, 0.01], recharge: [-0.5, 1.5, 0.01], spread: [-10, 20, 0.1], damage: [0, 200, 0.5], speed: [1, 120, 0.5] };
let editing = null;
function openEditor(id) {
  editing = id; switchTab("spells"); $("edit-spell").value = id; renderEditor();
}
function renderEditor() {
  const s = spell(editing ?? Number($("edit-spell").value));
  if (!s) return;
  editing = s.id;
  const change = state.draft.changes.find(c => c.id === s.id);
  $("edit-meta").textContent = `${s.class} · ${s.tag} · unlock ${s.unlock}${s.hint ? " · " + s.hint : ""}`;
  const value = f => f === "damage" ? s.base_damage : s[f];
  $("edit-fields").replaceChildren(...state.draft.fields.map(({ field, label, min }) => {
    const [lo, hi, step] = RANGES[field] ?? [0, 100, 0.1];
    const shipped = change?.fields[field]?.from;
    const range = el("input", { type: "range", min: min ?? lo, max: Math.max(hi, value(field)), step, value: value(field) });
    const num = el("input", { type: "number", step, value: value(field) });
    num.dataset.field = field;
    const box = el("div", { className: "edit-field" + (shipped != null ? " changed" : "") },
      el("span", { textContent: label }), el("div", { className: "pair" }, range, num),
      shipped != null ? el("span", { className: "muted", textContent: `shipped ${shipped}` }) : null);
    range.oninput = () => { num.value = range.value; box.classList.add("changed"); };
    num.oninput = () => { range.value = num.value; box.classList.add("changed"); };
    return box;
  }));
}
async function draftEdit(id, values) {
  status("Loading the draft table into a second lab process…");
  try {
    state.draft = await api("/api/wand-lab/draft", { spell: id, values });
    await afterDraft();
    status(state.draft.active ? "Draft scored natively" : "Draft cleared", "ok");
  } catch (error) { status(error.message, "error"); }
}
async function afterDraft() {
  await loadCatalog(); renderDraft(); renderEditor(); changed(false);
  if (state.survey) runSurvey();
  if (state.tab === "compare") renderCompare();
}
function renderDraft() {
  const d = state.draft;
  $("draft-banner").hidden = !d.active;
  $("draft-banner").textContent = d.active ? `Draft spell table active: ${d.changes.length} spell${d.changes.length === 1 ? "" : "s"} edited. Every number in the lab uses your draft; tiles also show the shipped difference. Save it in the Spell editor tab.` : "";
  $("draft-list").replaceChildren(...(d.changes.length ? d.changes.map(c => el("li", {},
    el("strong", { textContent: c.name }), " ",
    Object.entries(c.fields).map(([k, v]) => `${k} ${v.from} → ${v.to}`).join(", "), " ",
    el("button", { textContent: "Edit", onclick: () => openEditor(c.id) }))) : [el("li", { className: "muted", textContent: "No draft changes." })]));
  $("draft-apply").disabled = !d.active; $("draft-reset").disabled = !d.active;
}

// ---------- library, AI designer ----------
async function loadLibrary() {
  const { builds } = await api("/api/wand-lab/builds");
  const q = $("library-filter").value.toLowerCase();
  const shown = builds.filter(b => !q || b.name.toLowerCase().includes(q) || (b.notes ?? "").toLowerCase().includes(q));
  $("library").replaceChildren(...shown.map(b => el("li", {},
    el("strong", { textContent: b.name }), swatches(b.build.slots),
    el("div", { className: "muted small", textContent: `Tier ${b.build.tier} · ${state.catalog?.chassis[b.build.mode ?? 0]?.name ?? ""}` }),
    el("div", { className: "actions" },
      el("button", { textContent: "Load", onclick: () => load(b.build) }),
      el("button", { textContent: "Pin A", onclick: () => pinBuild(b.build) }),
      el("button", { textContent: "Compare", onclick: () => addCompare(b.name, b.build) }),
      el("button", { textContent: "✕", title: "Delete", onclick: async () => { if (confirm(`Delete ${b.name}?`)) { await api("/api/wand-lab/builds/delete", { name: b.name }); loadLibrary(); } } })))));
  if (!shown.length) $("library").append(el("li", { className: "muted small", textContent: builds.length ? "No builds match." : "No saved builds yet." }));
}

function swatches(slots) {
  return el("div", { className: "swatches" }, slots.filter(id => id !== EMPTY()).map(id =>
    el("span", { title: spell(id)?.name ?? "#" + id, style: { "--card": rgb(spell(id)?.color) } })));
}

async function load(build) {
  const tierChanged = build.tier !== state.build.tier;
  state.build = { ...state.build, ...structuredClone(build) };
  syncControls();
  if (tierChanged) await loadCatalog();
  normaliseSlots(); changed();
}

async function pinBuild(build) {
  try { const r = await api("/api/wand-lab/evaluate", { ...state.build, ...build }); state.pinned = r.current.summary; renderResults(); status("Baseline A set from saved build", "ok"); }
  catch (error) { status(error.message, "error"); }
}

const PRESETS = ["Endless sustain for long fights", "Maximum burst from a full pool", "Cheap crowd clearing", "Long-range sniper", "Fire build that never runs dry"];
async function suggest() {
  const goal = $("goal").value.trim();
  if (!goal) return status("Describe what the wand should be good at.", "error");
  $("suggest").disabled = true; status("AI designing… each proposal is scored natively");
  try {
    const out = await api("/api/wand-lab/suggest", { goal, build: state.build });
    $("suggestions").replaceChildren(...out.candidates.map(c => el("li", {},
      el("strong", { textContent: `${c.name}${c.round ? " (refined)" : ""}` }), swatches(c.build.slots),
      el("div", { className: "muted small", textContent: `${fmt(c.measured.dps)} dmg/s · ${fmt(c.measured.dps_30s)} 30 s avg · ${c.measured.sustainable ? "endless" : "dry in " + fmt(c.measured.seconds_to_empty) + " s"}` }),
      c.why ? el("div", { className: "small", textContent: c.why }) : null,
      el("div", { className: "actions" }, el("button", { textContent: "Load", onclick: () => load(c.build) }),
        el("button", { textContent: "Compare", onclick: () => addCompare(c.name, c.build) })))));
    status(out.note, "ok");
  } catch (error) { status(error.message, "error"); }
  finally { $("suggest").disabled = false; }
}

// ---------- share codes ----------
const encode = build => "WL1." + btoa(JSON.stringify({ t: build.tier, m: build.mode, r: build.rank, s: build.seed, d: build.slots })).replace(/=+$/, "");
function decode(code) {
  const m = /^WL1\.([A-Za-z0-9+/]+)$/.exec(code.trim());
  if (!m) throw new Error("Not a Wand Lab code");
  const padded = m[1] + "=".repeat((4 - m[1].length % 4) % 4);
  const o = JSON.parse(atob(padded));
  if (![1, 2, 3].includes(o.t) || !Array.isArray(o.d) || !o.d.every(Number.isInteger)) throw new Error("Damaged Wand Lab code");
  return { tier: o.t, mode: o.m | 0, rank: o.r | 0, seed: o.s | 0, slots: o.d.slice(0, 8) };
}

// ---------- controls ----------
function switchTab(tab) {
  state.tab = tab;
  for (const b of document.querySelectorAll("[data-tab]")) b.setAttribute("aria-selected", String(b.dataset.tab === tab));
  for (const p of document.querySelectorAll("[data-panel]")) p.hidden = p.dataset.panel !== tab;
  if (tab === "compare") renderCompare();
  if (tab === "perf") renderResults();
  if (tab === "spells") renderEditor();
}

function syncControls() {
  $("tier").value = state.build.tier; $("rank").value = state.build.rank; $("rank-out").value = state.build.rank;
  $("health").value = Math.round(state.build.health * 100); $("health-out").value = Math.round(state.build.health * 100) + "%";
  $("foe").checked = state.build.foe_visible; $("seed").value = state.build.seed;
  if (state.catalog) $("mode").value = state.build.mode;
}

async function loadCatalog() {
  state.catalog = await api("/api/wand-lab/spells?tier=" + state.build.tier);
  state.byId = new Map(state.catalog.spells.map(s => [s.id, s]));
  $("mode").replaceChildren(...state.catalog.chassis.map(c => el("option", { value: c.mode, textContent: `${c.mode} · ${c.name}${c.spells_per_cast > 1 ? ` (${c.spells_per_cast} per cast)` : ""}${c.mode >= 4 ? " binding" : ""}` })));
  $("mode").value = state.build.mode;
  const listed = state.catalog.spells.filter(s => s.palette !== "Hidden");
  const projectiles = listed.filter(s => s.class === "Projectile" && s.available);
  const ref = $("reference").value || "0";
  $("reference").replaceChildren(...projectiles.map(s => el("option", { value: s.id, textContent: s.name })));
  $("reference").value = projectiles.some(s => String(s.id) === ref) ? ref : String(projectiles[0]?.id ?? 0);
  const current = String(editing ?? listed[0]?.id);
  $("edit-spell").replaceChildren(...listed.map(s => el("option", { value: s.id, textContent: `${s.name} (${s.class})` })));
  $("edit-spell").value = current;
  renderClasses(); renderPalette();
}

function pin() { if (state.result) { state.pinned = state.result.summary; renderResults(); status("Baseline A pinned; tiles now show deltas", "ok"); } }

function wire() {
  $("tier").onchange = async e => { state.build.tier = Number(e.target.value); await loadCatalog(); normaliseSlots(); changed(); };
  $("mode").onchange = e => { state.build.mode = Number(e.target.value); changed(); };
  $("rank").oninput = e => { state.build.rank = Number(e.target.value); $("rank-out").value = e.target.value; changed(); };
  $("health").oninput = e => { state.build.health = Math.max(0.01, Number(e.target.value) / 100); $("health-out").value = e.target.value + "%"; changed(); };
  $("foe").onchange = e => { state.build.foe_visible = e.target.checked; changed(); };
  $("seed").onchange = e => { state.build.seed = Math.max(0, Math.floor(Number(e.target.value) || 0)); changed(); };
  $("reroll").onclick = () => { state.build.seed = Math.floor(Math.random() * 1e9) + 1; syncControls(); changed(); };
  $("search").oninput = e => { state.query = e.target.value; renderPalette(); };
  $("sort").onchange = e => { state.sort = e.target.value; renderPalette(); };
  $("castable").onchange = e => { state.castableOnly = e.target.checked; renderPalette(); };
  $("clear").onclick = () => { state.build.slots = state.build.slots.map((s, i) => state.locked.includes(i) ? s : EMPTY()); changed(); };
  $("fill").onclick = () => {
    const last = [...state.build.slots].reverse().find(s => s !== EMPTY());
    if (last == null) return;
    state.build.slots = state.build.slots.map(s => s === EMPTY() ? last : s); changed();
  };
  $("pin").onclick = pin;
  $("undo").onclick = () => travel(state.undo, state.redo);
  $("redo").onclick = () => travel(state.redo, state.undo);
  $("share").onclick = async () => {
    const code = encode(state.build);
    const link = `${location.origin}/wand-lab?code=${encodeURIComponent(code)}`;
    try { await navigator.clipboard.writeText(code); status(`Code copied: ${code} · link (add the session #key): ${link}`, "ok"); } catch { prompt("Copy this build code", code); }
  };
  $("import").onclick = () => { const code = prompt("Paste a Wand Lab code (WL1.…)"); if (!code) return; try { load(decode(code)); status("Build loaded from code", "ok"); } catch (error) { status(error.message, "error"); } };
  $("help").onclick = () => $("help-dialog").showModal();
  $("equip").onclick = async () => {
    try { const r = await api("/api/wand-lab/equip", state.build); status(`Minted into hotbar slot ${r.data?.equipped?.hotbar_slot ?? "?"} of the playtest world`, "ok"); }
    catch (error) { status(error.message + " — start a playtest world first (Playtest → Engine workbench)", "error"); }
  };
  $("save").onclick = async () => {
    try { await api("/api/wand-lab/builds", { name: $("build-name").value, build: state.build }); $("build-name").value = ""; loadLibrary(); status("Build saved", "ok"); }
    catch (error) { status(error.message, "error"); }
  };
  $("library-filter").oninput = () => loadLibrary();
  $("suggest").onclick = suggest;
  $("goal-presets").replaceChildren(...PRESETS.map(p => el("button", { textContent: p, onclick: () => { $("goal").value = p; } })));
  for (const b of document.querySelectorAll("[data-tab]")) b.onclick = () => switchTab(b.dataset.tab);
  $("enemy-filter").oninput = renderKills; $("targets").oninput = renderKills;
  $("compare-add").onclick = () => addCompare(`Deck ${state.compare.length + 1}`, state.build);
  $("compare-clear").onclick = () => { state.compare = []; persist(); renderCompare(); };
  $("optimize").onclick = optimize;
  $("survey").onclick = runSurvey;
  $("balance-ai").onclick = balanceAI;
  $("edit-spell").onchange = e => { editing = Number(e.target.value); renderEditor(); };
  $("draft-score").onclick = () => {
    const values = Object.fromEntries([...$("edit-fields").querySelectorAll("input[type=number]")].map(i => [i.dataset.field, Number(i.value)]));
    draftEdit(editing, values);
  };
  $("draft-revert").onclick = () => {
    const change = state.draft.changes.find(c => c.id === editing);
    if (!change) return status("This spell has no draft changes.");
    draftEdit(editing, Object.fromEntries(Object.entries(change.fields).map(([k, v]) => [k, v.from])));
  };
  $("draft-reset").onclick = async () => { if (!confirm("Discard every draft spell change?")) return; state.draft = await api("/api/wand-lab/draft/reset", {}); await afterDraft(); status("Draft discarded", "ok"); };
  $("draft-apply").onclick = async () => {
    if (!confirm("Write the draft into assets/spells.ron? A backup is kept; rebuild to ship it in game.")) return;
    try { const r = await api("/api/wand-lab/draft/apply", {}); status(r.note, "ok"); } catch (error) { status(error.message, "error"); }
  };
  addEventListener("resize", () => { if (state.tab === "perf") renderResults(); if (state.tab === "compare") renderCompare(); });
  addEventListener("keydown", e => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName);
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !typing) { e.preventDefault(); e.shiftKey ? travel(state.redo, state.undo) : travel(state.undo, state.redo); return; }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y" && !typing) { e.preventDefault(); travel(state.redo, state.undo); return; }
    if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "/") { e.preventDefault(); $("search").focus(); }
    if (e.key === "?") $("help-dialog").showModal();
    if (e.key === "p") pin();
    const tabs = ["perf", "compare", "optimize", "balance", "spells"];
    if (/^[1-5]$/.test(e.key)) switchTab(tabs[Number(e.key) - 1]);
  });
}

(async () => {
  wire(); syncControls();
  try {
    state.draft = await api("/api/wand-lab/draft");
    // Shareable links: /wand-lab?code=WL1.…&tab=balance#<session>
    const params = new URLSearchParams(location.search);
    if (params.get("code")) { try { Object.assign(state.build, decode(params.get("code"))); syncControls(); } catch (error) { status(error.message, "error"); } }
    await loadCatalog();
    normaliseSlots(); remember(); renderDraft(); renderDeck(); changed(false); loadLibrary();
    if (["perf", "compare", "optimize", "balance", "spells"].includes(params.get("tab"))) switchTab(params.get("tab"));
    if (params.get("tab") === "balance") runSurvey();
    api("/api/wand-lab/bestiary").then(b => { state.enemies = b.enemies; if (state.result) renderKills(); }).catch(() => {});
  } catch (error) { status(error.message, "error"); }
})();
