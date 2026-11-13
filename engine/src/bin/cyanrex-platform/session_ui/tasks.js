import { parseIndex, parseContent } from './task-contract.js';
import { createMetadataEditor } from './task-edit.js';
import { platformRequest } from './request.js';

// Owner-private read view; the separate editor alone owns metadata PUT and unsaved work.
export function createTaskReader({ run, isActive, sessionRejected, label }) {
  const byId = id => document.getElementById(id);
  const load = byId("load-tasks"), next = byId("next-tasks"), list = byId("task-list");
  const state = byId("task-read-state"), title = byId("task-content-title");
  const metadata = byId("task-content-metadata"), select = byId("task-payload-select"), text = byId("task-text");
  let busy = true, generation = 0, controller = null, entries = [], cursor = null, scope = null;
  let snapshot = null, stateKey = "tasksIdle";
  const editor = createMetadataEditor({ run, isActive, sessionRejected, label, onSaved(value, bodySaved = false) {
    snapshot = value;
    entries = entries.map(entry => entry.reference.task_id !== value.task.reference.task_id ? entry
      : Object.freeze({ reference: value.task.reference, revision: value.task.revision, title: value.task.title,
        status: value.task.status, definition_ref: null, updated_at: value.task.updated_at,
        payload_items: value.manifest.payload.length }));
    renderList(); renderDetail(); observe("opened", bodySaved ? "bodyCachedContent" : "metaCachedContent");
  } });

  function controls() {
    load.disabled = busy || !isActive();
    next.disabled = busy || !isActive() || !cursor;
    select.disabled = busy || !isActive() || !snapshot?.contents.length;
    for (const button of list.querySelectorAll("button")) button.disabled = busy || !isActive();
  }
  function observe(value, key) {
    state.dataset.state = value;
    state.dataset.i18n = key;
    stateKey = key;
    state.textContent = label(key);
  }
  function clearDetail() {
    editor.invalidate();
    snapshot = null;
    title.textContent = "";
    metadata.textContent = "";
    select.replaceChildren();
    text.textContent = "";
    byId("task-content").hidden = true;
  }
  function clearList() {
    entries = [];
    cursor = null;
    scope = null;
    list.replaceChildren();
  }
  function clear() {
    generation++;
    controller?.abort();
    controller = null;
    clearDetail();
    clearList();
    observe("idle", "tasksIdle");
    controls();
  }

  async function read(path) {
    const own = new AbortController();
    controller = own;
    try { return await platformRequest(path, "GET", { signal: own.signal }); }
    finally {
      if (controller === own) controller = null;
    }
  }

  function rejectResponse(response) {
    if (response.status === 200) return false;
    clearDetail();
    clearList();
    if (response.status === 401 || response.status === 403) sessionRejected(response);
    observe(response.status === 404 ? "unavailable" : "unknown",
      response.status === 404 ? "tasksUnavailable" : "tasksUnknown");
    return true;
  }
  function failed(error) {
    clearDetail();
    clearList();
    if (error?.deniedStatus === 401 || error?.deniedStatus === 403) {
      sessionRejected({ status: error.deniedStatus, raw: "" });
    }
    observe("unknown", "tasksUnknown");
  }

  function renderList() {
    list.replaceChildren();
    for (const entry of entries) {
      const row = document.createElement("li"), button = document.createElement("button");
      button.type = "button";
      button.className = "task-entry secondary";
      button.dataset.taskId = entry.reference.task_id;
      const name = document.createElement("span"), info = document.createElement("span");
      name.className = "task-entry-title";
      name.textContent = entry.title;
      info.className = "hint";
      info.textContent = `${label(`taskStatus_${entry.status}`)} · ${label("taskRevision")} ${entry.revision} · ${label("taskPayloadCount")} ${entry.payload_items}`;
      button.append(name, info);
      button.addEventListener("click", () => editor.leave(() => open(entry)));
      row.append(button);
      list.append(row);
    }
    controls();
  }
  function renderDetail() {
    if (!snapshot) return;
    const selected = select.selectedIndex;
    title.textContent = snapshot.task.title;
    metadata.textContent = `${label(`taskStatus_${snapshot.task.status}`)} · ${label("taskRevision")} ${snapshot.task.revision}`;
    select.replaceChildren();
    snapshot.manifest.payload.forEach((item, index) => {
      const option = document.createElement("option");
      option.value = String(index);
      option.textContent = `${item.filename} · ${item.language}`;
      select.append(option);
    });
    if (snapshot.contents.length) {
      select.selectedIndex = selected >= 0 && selected < snapshot.contents.length ? selected : 0;
      text.textContent = snapshot.contents[select.selectedIndex].text;
    } else text.textContent = "";
    byId("task-content").hidden = false;
    controls();
  }

  function loadPage(after = null) {
    if (busy || !isActive()) return;
    const expectedScope = after ? scope : null;
    void run(async parentCurrent => {
      const epoch = ++generation;
      const current = () => parentCurrent() && generation === epoch && isActive();
      clearDetail();
      clearList();
      observe("loading", "tasksLoading");
      try {
        const response = await read(`/platform/v1/task-index?limit=20${after ? `&after=${after}` : ""}`);
        if (!current() || rejectResponse(response)) return;
        const page = parseIndex(response.raw, { after, scope: expectedScope, limit: 20 });
        if (!current()) return;
        entries = page.entries;
        scope = expectedScope || entries[0]?.reference.workspace || null;
        cursor = page.next_after?.task_id || null;
        renderList();
        observe(entries.length ? "listed" : "empty", entries.length ? "tasksListed" : "tasksEmptyList");
      } catch (error) {
        if (current()) failed(error);
      }
    });
  }
  function open(entry) {
    if (busy || !isActive() || !entries.includes(entry)) return;
    const selectedScope = scope;
    void run(async parentCurrent => {
      const epoch = ++generation;
      const current = () => parentCurrent() && generation === epoch && isActive();
      clearDetail();
      observe("loading", "tasksLoading");
      try {
        // Content can be much larger than index metadata; decoding/hashing completes before render.
        const response = await read(`/platform/v1/tasks/${entry.reference.task_id}`);
        if (!current() || rejectResponse(response)) return;
        const value = await parseContent(response.raw, entry, selectedScope);
        if (!current()) return;
        snapshot = value;
        editor.acceptSnapshot(value);
        renderDetail();
        observe(value.contents.length ? "opened" : "empty", value.contents.length ? "tasksOpened" : "tasksEmptyPayload");
      } catch (error) {
        if (current()) failed(error);
      }
    });
  }

  load.addEventListener("click", () => editor.leave(() => loadPage()));
  next.addEventListener("click", () => { if (cursor) editor.leave(() => loadPage(cursor)); });
  select.addEventListener("change", () => {
    if (busy || !isActive() || !snapshot) return;
    text.textContent = snapshot.contents[select.selectedIndex]?.text || "";
  });
  clear();
  return Object.freeze({ clear, leave: editor.leave, hasWork: editor.hasWork,
    setBusy(value) { busy = value; editor.setBusy(value); controls(); },
    translate() { state.textContent = label(stateKey); renderList(); renderDetail(); editor.translate(); } });
}
