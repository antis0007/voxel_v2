import { el, button } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";
import { canAssist } from "./agent-contract.mjs";

const statuses = ["Ready", "In progress", "Blocked", "Review", "Done"];
const priorities = ["Urgent", "High", "Normal", "Low"];
const route = "/api/backlog";
const activityLabels = {
  create: "Created", edit: "Edited", claim: "Claimed", assign: "Assigned",
  release: "Released", block: "Blocked", unblock: "Unblocked", handoff: "Handed off",
  verify: "Verified", complete: "Completed", reopen: "Reopened",
};
const label = (name, input) => el("label", {}, name, input);
const picker = (values, value) => {
  const select = el("select", {}, values.map(item => el("option", { value: item }, item)));
  select.value = value;
  return select;
};
const line = (value = "", maxLength = 160) => el("input", { required: true, value, maxLength });
const notes = (value = "") => el("textarea", { required: true, value, maxLength: 4000, rows: 4 });
const scope = (paths = []) => el("textarea", { value: paths.join("\n"), maxLength: 3000, rows: 2 });
const changeReference = ref => {
  if (!/^https?:\/\//i.test(ref)) return el("code", {}, ref);
  return el("a", { href: ref, target: "_blank", rel: "noopener noreferrer" }, ref);
};

export function mountBacklog(host, api, openAi) {
  let tasks = [], selected, detail, refreshing = false, saving = false;
  const actor = el("input", { placeholder: "Your name", maxLength: 100 });
  const actorKey = "axiom-backlog-actor";
  actor.value = sessionStorage.getItem(actorKey) ?? localStorage.getItem(actorKey) ?? "";
  const search = el("input", { type: "search", placeholder: "Search tasks", "aria-label": "Search tasks" });
  const status = picker(["All statuses", ...statuses], "All statuses");
  status.setAttribute("aria-label", "Filter by status");
  const owner = picker([], "");
  owner.setAttribute("aria-label", "Filter by owner");
  const notice = el("p", { role: "status", className: "backlog-notice" });
  const counts = el("div", { className: "backlog-counts", "aria-label": "Task counts" });
  const list = el("div", { className: "backlog-list", "aria-label": "Tasks" });
  const inspector = el("section", { className: "backlog-detail", "aria-label": "Task details" });
  const dialog = el("dialog", { className: "backlog-dialog", "aria-labelledby": "backlog-dialog-title" });
  host.append(
    el("header", { className: "backlog-header" }, el("h1", {}, "Backlog"),
      label("Acting as", actor), button("Refresh", () => {
        notice.textContent = "";
        refresh();
      }),
      button("New task", () => editor("create"), "", "primary")),
    counts, notice,
    el("div", { className: "backlog-body" },
      el("section", { className: "backlog-queue", "aria-label": "Task queue" },
        el("div", { className: "backlog-filters" }, search, status, owner), list), inspector),
    dialog);
  renderDetail();
  actor.oninput = () => {
    sessionStorage.setItem(actorKey, actor.value);
    renderList();
    renderDetail();
  };
  search.oninput = status.onchange = owner.onchange = renderList;
  setInterval(() => { if (!host.hidden && !document.hidden) refresh(); }, 5000);

  async function refresh() {
    if (refreshing || saving) return;
    refreshing = true;
    try {
      const incoming = await api(route);
      if (saving) return;
      const revisions = new Map(tasks.map(task => [task.id, task.revision]));
      if (incoming.length !== tasks.length || incoming.some(row => revisions.get(row.id) !== row.revision)) {
        tasks = incoming;
        renderList();
      } else if (!tasks.length && !list.childElementCount) renderList();
      if (selected && tasks.find(row => row.id === selected)?.revision !== detail?.revision) {
        await show(selected);
      }
    } catch (error) { notice.textContent = error.message; }
    finally { refreshing = false; }
  }

  async function show(id) {
    if (selected !== id) notice.textContent = "";
    selected = id;
    if (detail?.id !== id) {
      detail = undefined;
      inspector.replaceChildren(el("p", { role: "status", className: "backlog-meta" }, "Loading task…"));
    }
    for (const control of list.querySelectorAll("button")) {
      control.setAttribute("aria-current", String(control.dataset.task === id));
    }
    try {
      const row = await api(route, id);
      if (selected !== id || (detail?.id === id && detail.revision >= row.revision)) return;
      detail = row;
      renderDetail();
    } catch (error) { notice.textContent = error.message; }
  }

  function renderList() {
    const query = search.value.trim().toLowerCase();
    const previousOwner = owner.value;
    const owners = [...new Set(tasks.map(task => task.owner).filter(Boolean))].sort((a, b) => a.localeCompare(b));
    owner.replaceChildren(el("option", { value: "" }, "All owners"),
      el("option", { value: "mine" }, "Mine"), el("option", { value: "unassigned" }, "Unassigned"),
      ...owners.map(name => el("option", { value: "owner:" + name }, name)));
    if ([...owner.options].some(option => option.value === previousOwner)) owner.value = previousOwner;
    const ownerMatch = owner.value === "mine" ? actor.value.trim()
      : owner.value === "unassigned" ? "" : owner.value.slice(6);
    const visible = tasks.filter(task =>
      (status.selectedIndex === 0 || task.status === status.value) &&
      (!owner.value || (task.owner === ownerMatch && (owner.value !== "mine" || !!ownerMatch))) &&
      (!query || [task.title, task.owner, task.blocker, task.id, ...task.paths].some(value => value?.toLowerCase().includes(query))))
      .sort((a, b) => priorities.indexOf(a.priority) - priorities.indexOf(b.priority)
        || b.updatedAt.localeCompare(a.updatedAt));
    const focused = list.contains(document.activeElement) ? document.activeElement.dataset.task : undefined;
    counts.replaceChildren(...statuses.map(value => {
      const filter = button(value + " " + tasks.filter(task => task.status === value).length, () => {
        status.value = status.value === value ? "All statuses" : value;
        renderList();
        counts.children[statuses.indexOf(value)].focus();
      });
      filter.setAttribute("aria-pressed", String(status.value === value));
      return filter;
    }));
    list.replaceChildren(...visible.map(task => {
      const control = button("", () => show(task.id), "", "backlog-row");
      control.dataset.task = task.id;
      control.setAttribute("aria-current", String(task.id === selected));
      control.append(el("strong", {}, task.title),
        el("span", { className: "backlog-meta" }, [task.status, task.priority, task.owner || "Unassigned",
          task.verification].filter(Boolean).join(" · ")));
      if (task.blocker) control.append(el("span", { className: "backlog-blocker" }, task.blocker));
      return control;
    }));
    if (!visible.length) list.append(el("p", { className: "backlog-meta" }, tasks.length ? "No tasks match these filters." : "No tasks yet."));
    if (selected && !visible.some(task => task.id === selected)) {
      list.append(button("Show selected task", () => {
        search.value = "";
        status.selectedIndex = 0;
        owner.value = "";
        renderList();
        [...list.querySelectorAll("[data-task]")].find(row => row.dataset.task === selected)?.focus();
      }));
    }
    if (focused) [...list.children].find(row => row.dataset.task === focused)?.focus({ preventScroll: true });
  }

  function context(task) {
    return [
      task.title, `Backlog task ${task.id} at revision ${task.revision}`,
      "Acceptance criteria\n" + task.acceptance,
      task.paths.length && "Source paths\n" + task.paths.join("\n"),
      task.blocker && "Blocker\n" + task.blocker,
      task.handoff && "Current handoff\n" + task.handoff.ref + "\n" + task.handoff.note,
      task.verification && "Verification\n" + task.verification.result + "\n" + task.verification.note,
    ].filter(Boolean).join("\n\n");
  }

  function renderDetail() {
    const task = detail;
    if (!task) {
      inspector.replaceChildren(el("p", { className: "backlog-meta" }, "Select a task to see its acceptance criteria and activity."));
      return;
    }
    const mine = task.owner && task.owner === actor.value.trim();
    const review = task.status === "Review", blocked = task.status === "Blocked";
    const action = (name, caption, immediate = false) =>
      button(caption, () => immediate ? submit(name, task) : editor(name, task));
    const controls = task.status === "Done" ? [action("reopen", "Reopen")] : [
      !task.owner && !blocked ? action("claim", "Claim task", true) : null,
      action("assign", task.owner ? "Reassign" : "Assign"),
      mine ? action("release", "Release", true) : null,
      blocked ? action("unblock", "Clear blocker", true) : null,
      mine ? action("block", blocked ? "Update blocker" : "Report blocker") : null,
      mine && !blocked ? action("handoff", "Hand off changes") : null,
      review ? action("verify", "Record verification") : null,
      review && task.verification?.result === "Pass" ? action("complete", "Mark done", true) : null,
      action("edit", "Edit task"),
    ];
    const section = (heading, ...children) => el("section", { className: "backlog-section" },
      el("h3", {}, heading), children);
    const copy = button("Copy task context", async () => {
      try { await copyText(context(task)); notice.textContent = "Task context copied."; }
      catch (error) { notice.textContent = error.message; }
    });
    inspector.replaceChildren(...[
      el("header", {}, el("h2", { tabIndex: -1 }, task.title), el("p", { className: "backlog-meta" },
        [task.status, task.priority, task.owner || "Unassigned"].join(" · "))),
      el("div", { className: "backlog-actions" }, controls),
      task.blocker ? section("Blocker", el("p", { className: "backlog-blocker" }, task.blocker)) : null,
      section("Acceptance criteria", el("p", {}, task.acceptance)),
      task.paths.length ? section("Source paths", el("p", { className: "backlog-meta" }, task.paths.join("\n"))) : null,
      task.handoff ? section("Change handoff", changeReference(task.handoff.ref), el("p", {}, task.handoff.note),
        el("p", { className: "backlog-meta" }, task.verification ? "Latest verification · " + task.verification.result : "Awaiting verification"),
        task.verification ? el("p", {}, task.verification.note) : null) : null,
      el("div", { className: "backlog-actions" }, copy, button("Open in AI",
        () => openAi({ task: context(task), paths: task.paths.filter(canAssist), origin: "Backlog" }))),
      section("Activity", el("ol", { className: "backlog-history" }, task.history.slice().reverse().map(event =>
        el("li", {}, el("strong", {}, activityLabels[event.action] + (event.result ? " · " + event.result : "")),
          el("span", { className: "backlog-meta" }, event.actor + " · " + new Date(event.at).toLocaleString()
            + (event.action === "assign" ? " · " + event.owner : "")),
          event.ref ? el("code", {}, event.ref) : null,
          event.note ? el("p", {}, event.note) : null,
          event.acceptance ? el("p", {}, [event.title, event.priority, event.acceptance, ...event.paths].join("\n")) : null))))
    ].filter(Boolean));
  }

  function editor(action, task) {
    if (!actor.value.trim()) {
      notice.textContent = "Enter your name in Acting as before changing the backlog.";
      actor.focus();
      return;
    }
    const fields = {
      create: () => ({ title: ["Title", line()], priority: ["Priority", picker(priorities, "Normal")],
        acceptance: ["Acceptance criteria", notes()], paths: ["Source paths (optional, one per line)", scope()] }),
      edit: () => ({ title: ["Title", line(task.title)], priority: ["Priority", picker(priorities, task.priority)],
        acceptance: ["Acceptance criteria", notes(task.acceptance)], paths: ["Source paths (optional, one per line)", scope(task.paths)] }),
      assign: () => ({ owner: ["New owner", line(task.owner, 100)] }),
      block: () => ({ note: ["What is blocking the work?", notes(task.blocker)] }),
      handoff: () => ({ ref: ["Commit, branch, bundle, or PR", line("", 500)], note: ["Changes and remaining work", notes()] }),
      verify: () => ({ result: ["Result", picker(["", "Pass", "Fail"], "")],
        note: ["Checks run, results, and evidence references", notes()] }),
      reopen: () => ({ note: ["Reason for reopening", notes()] }),
    }[action]();
    const titles = { create: "New task", edit: "Edit task", assign: "Assign owner", block: "Record blocker",
      handoff: "Hand off changes", verify: "Record verification", reopen: "Reopen task" };
    const error = el("p", { role: "alert", className: "backlog-notice" });
    const save = el("button", { type: "submit", className: "primary" }, action === "create" ? "Create task" : "Save");
    if (fields.result) fields.result[1].required = true;
    const form = el("form", {},
      el("h2", { id: "backlog-dialog-title" }, titles[action]),
      task ? el("p", { className: "backlog-meta" }, task.title) : null,
      ...Object.values(fields).map(([name, input]) => label(name, input)), error,
      el("div", { className: "backlog-actions" }, save, button("Cancel", () => dialog.close())));
    form.onsubmit = async event => {
      event.preventDefault();
      save.disabled = true;
      const data = Object.fromEntries(Object.entries(fields).map(([key, [, input]]) => [key, input.value]));
      if (await submit(action, task, data, error)) {
        dialog.close();
        inspector.querySelector("h2")?.focus({ preventScroll: true });
      }
      else save.disabled = false;
    };
    dialog.replaceChildren(form);
    dialog.showModal();
  }

  async function submit(action, task, fields = {}, feedback = notice) {
    if (saving) return false;
    const name = actor.value.trim();
    if (!name) {
      feedback.textContent = "Enter your name in Acting as before changing the backlog.";
      if (feedback === notice) actor.focus();
      return false;
    }
    saving = true;
    try {
      const row = await api(route, undefined, { action, actor: name, id: task?.id, revision: task?.revision, ...fields });
      selected = row.id;
      detail = row;
      tasks = [...tasks.filter(item => item.id !== row.id), { ...row, verification: row.verification?.result }];
      if (action === "create") {
        search.value = "";
        status.selectedIndex = 0;
        owner.value = "";
      }
      renderList();
      renderDetail();
      notice.textContent = activityLabels[action] + " · " + row.title;
      return true;
    } catch (error) {
      feedback.textContent = error.message;
      return false;
    } finally {
      saving = false;
      refresh();
    }
  }

  return { refresh };
}
