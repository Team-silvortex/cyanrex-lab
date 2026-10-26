import { prepareMetadataSave, parseMetadataSaved } from './task-contract.js';

// One page-memory metadata editor. Existing bytes/pins/order are never edited or published.
export function createMetadataEditor({ run, isActive, sessionRejected, label, onSaved }) {
  const byId = id => document.getElementById(id);
  const start = byId("edit-task"), panel = byId("task-metadata-editor"), state = byId("task-edit-state");
  const title = byId("task-edit-title"), item = byId("task-edit-item");
  const filename = byId("task-edit-filename"), language = byId("task-edit-language");
  const save = byId("save-task-metadata"), discard = byId("discard-task-edit");
  const confirmation = byId("save-metadata-dialog"), leaving = byId("task-leave-dialog");
  let baseline = null, draft = null, attempt = null, staged = null, departure = null;
  let busy = true, editing = false, dirty = false, quarantined = false, generation = 0;
  let phase = "clean", stateKey = "metaClean", controller = null, collectingDenial = false;
  const held = () => dirty || attempt !== null || quarantined;
  const terminal = () => attempt !== null;
  const copy = value => ({ title: value.title,
    items: value.payload.map(binding => ({ filename: binding.filename, language: binding.language })) });

  function observe(value, key) {
    phase = value; stateKey = key;
    state.dataset.state = value; state.dataset.i18n = key;
    state.textContent = label(key);
  }
  function controls() {
    start.hidden = editing && !quarantined;
    start.disabled = busy || !isActive() || !baseline || baseline.task.status !== "draft" || held();
    panel.hidden = !editing || quarantined;
    discard.hidden = !editing && !held();
    discard.disabled = busy;
    for (const field of [title, item, filename, language]) field.disabled = busy || !isActive() || quarantined;
    title.readOnly = terminal(); filename.readOnly = terminal(); language.readOnly = terminal();
    item.disabled ||= !draft?.items.length;
    filename.disabled ||= !draft?.items.length;
    language.disabled ||= !draft?.items.length;
    save.disabled = busy || !isActive() || quarantined || !editing || !dirty || terminal();
    for (const id of ["confirm-save-metadata", "cancel-save-metadata", "confirm-task-leave", "cancel-task-leave"])
      byId(id).disabled = busy;
  }
  function emptyDom() {
    title.value = ""; item.replaceChildren(); filename.value = ""; language.value = "";
    byId("metadata-save-target").textContent = "";
    byId("metadata-save-summary").textContent = "";
    byId("task-leave-target").textContent = "";
  }
  function closeDialogs() {
    if (confirmation.open) confirmation.close();
    if (leaving.open) leaving.close();
    staged = null; departure = null;
  }
  function reset(keepBaseline = false) {
    generation++;
    controller?.abort(); controller = null;
    closeDialogs(); emptyDom();
    if (!keepBaseline) baseline = null;
    draft = null; attempt = null; editing = false; dirty = false; quarantined = false;
    observe("clean", "metaClean"); controls();
  }
  function invalidate() {
    if (!held()) { reset(); return; }
    generation++;
    // A known denial hides private DOM immediately. Only that same request may finish its
    // bounded error envelope; another context change still aborts it and fences its result.
    if (!collectingDenial) { controller?.abort(); controller = null; }
    closeDialogs(); emptyDom();
    // Keep only the closed metadata buffer/attempt. No cross-Session restore is implemented.
    baseline = null; editing = false; quarantined = true;
    observe("quarantined", "metaQuarantined"); controls();
  }
  function renderDraft() {
    if (!draft || quarantined) return;
    const selected = Math.max(0, item.selectedIndex);
    title.value = draft.title;
    item.replaceChildren();
    draft.items.forEach((binding, index) => {
      const option = document.createElement("option");
      option.value = String(index); option.textContent = `${index + 1}. ${binding.filename}`;
      item.append(option);
    });
    if (draft.items.length) {
      item.selectedIndex = Math.min(selected, draft.items.length - 1);
      filename.value = draft.items[item.selectedIndex].filename;
      language.value = draft.items[item.selectedIndex].language;
    } else { filename.value = ""; language.value = ""; }
    controls();
  }
  function inputChanged() {
    if (!baseline || !draft || quarantined) return;
    draft.title = title.value;
    const selected = draft.items[item.selectedIndex];
    if (selected) { selected.filename = filename.value; selected.language = language.value; }
    generation++;
    dirty = JSON.stringify(draft) !== JSON.stringify(copy(baseline.manifest));
    if (!terminal()) observe(dirty ? "dirty" : "clean", dirty ? "metaDirty" : "metaClean");
    controls();
  }
  for (const field of [title, filename, language]) field.addEventListener("input", inputChanged);
  item.addEventListener("change", () => {
    if (!draft || quarantined || busy) return;
    const selected = draft.items[item.selectedIndex];
    filename.value = selected?.filename || ""; language.value = selected?.language || "";
  });

  function target() {
    return !baseline || quarantined ? label("metaLockedTarget")
      : `${baseline.task.title} · ${baseline.task.reference.task_id} · ${label("taskRevision")} ${baseline.task.revision}`;
  }
  function leave(action, reason = "navigate") {
    if (busy || confirmation.open || leaving.open) return;
    if (!held()) { action(); return; }
    departure = { generation, action };
    byId("task-leave-target").textContent = target();
    byId("task-leave-warning").textContent = label(terminal() || quarantined ? "metaLeaveUnknown" : "metaLeaveDirty");
    byId("task-leave-action").textContent = label(reason === "logout" ? "metaLeaveLogout"
      : reason === "login" ? "metaLeaveLogin" : "metaLeaveAction");
    leaving.showModal(); byId("cancel-task-leave").focus();
  }
  const cancelLeave = () => { if (!busy) { departure = null; leaving.close(); discard.focus(); } };
  byId("cancel-task-leave").addEventListener("click", cancelLeave);
  leaving.addEventListener("cancel", event => { event.preventDefault(); cancelLeave(); });
  byId("confirm-task-leave").addEventListener("click", () => {
    if (busy || !leaving.open || !departure) return;
    const selected = departure;
    departure = null; leaving.close();
    if (selected.generation !== generation) { observe(phase, stateKey); return; }
    reset(Boolean(baseline));
    selected.action();
  });
  discard.addEventListener("click", () => {
    if (busy) return;
    if (held()) leave(() => {}, "cancel");
    else reset(Boolean(baseline));
  });
  start.addEventListener("click", () => {
    if (busy || !isActive() || !baseline || baseline.task.status !== "draft" || held()) return;
    draft = copy(baseline.manifest); editing = true;
    observe("clean", "metaClean"); renderDraft(); title.focus();
  });

  const ws = "[\\x20\\x09\\x0a\\x0d]*";
  const field = (key, value) => `"${key}"${ws}:${ws}${value}`;
  const object = (first, second) => `\\{${ws}(?:${first}${ws},${ws}${second}|${second}${ws},${ws}${first})${ws}\\}`;
  const conflict = new RegExp(`^${ws}${object(field("schema_version", "1"), field("error",
    object(field("code", '"(?:conflict|stale_revision|invalid_transition)"'), field("outcome", '"unconfirmed"'))))}${ws}$`);

  async function write(descriptor, onDenial) {
    const own = new AbortController(); controller = own;
    let deniedStatus = null, rejectAbort;
    const interrupted = new Promise((_resolve, reject) => { rejectAbort = reject; });
    const abort = () => rejectAbort(new Error("taskMetadata.unknown"));
    own.signal.addEventListener("abort", abort, { once: true });
    // This is a browser wait bound, never a server transaction deadline or rollback proof.
    const timer = setTimeout(() => own.abort(), 10_000);
    const work = (async () => {
      const response = await fetch(`/platform/v1/tasks/${descriptor.task_id}/content`, {
        method: "PUT", mode: "same-origin", credentials: "same-origin", cache: "no-store",
        redirect: "error", signal: own.signal, headers: { "Content-Type": "application/json",
          "X-Cyanrex-Platform-Request": "1" }, body: JSON.stringify(descriptor.body),
      });
      if (own.signal.aborted) { void response.body?.cancel().catch(() => undefined); own.signal.throwIfAborted(); }
      if (response.status === 401 || response.status === 403) {
        deniedStatus = response.status;
        onDenial(response.status);
      }
      if (!/^application\/json(?:;\s*charset=utf-8)?$/i.test(response.headers.get("content-type") || "") || !response.body) {
        void response.body?.cancel().catch(() => undefined); throw new Error("taskMetadata.unknown");
      }
      const reader = response.body.getReader(), chunks = [];
      let size = 0, frames = 0;
      const cancel = () => { void reader.cancel().catch(() => undefined); };
      own.signal.addEventListener("abort", cancel, { once: true });
      try {
        while (true) {
          const { value, done } = await reader.read(); own.signal.throwIfAborted();
          if (done) break;
          size += value.byteLength;
          if (++frames > 4096 || size > 128 * 1024) { cancel(); throw new Error("taskMetadata.unknown"); }
          chunks.push(value);
        }
        const bytes = new Uint8Array(size); let offset = 0;
        for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
        return { status: response.status, raw: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
      } finally { own.signal.removeEventListener("abort", cancel); reader.releaseLock(); }
    })();
    try { return await Promise.race([work, interrupted]); }
    catch { const error = new Error("taskMetadata.unknown"); error.deniedStatus = deniedStatus; throw error; }
    finally {
      clearTimeout(timer); own.signal.removeEventListener("abort", abort);
      if (controller === own) controller = null;
    }
  }

  save.addEventListener("click", () => {
    if (busy || !isActive() || !editing || !dirty || terminal() || quarantined || confirmation.open) return;
    try {
      const descriptor = prepareMetadataSave(baseline, draft);
      staged = { descriptor, generation, baseline };
      byId("metadata-save-target").textContent = target();
      byId("metadata-save-summary").textContent = `${label("metaTitle")}: ${descriptor.body.manifest.title}\n`
        + descriptor.body.manifest.payload.map((binding, index) => `${index + 1}. ${binding.filename} · ${binding.language}`).join("\n");
      confirmation.showModal(); byId("cancel-save-metadata").focus();
    } catch { observe("invalid-local", "metaInvalid"); controls(); }
  });
  const cancelSave = () => { if (!busy) { staged = null; confirmation.close(); save.focus(); } };
  byId("cancel-save-metadata").addEventListener("click", cancelSave);
  confirmation.addEventListener("cancel", event => { event.preventDefault(); cancelSave(); });
  byId("confirm-save-metadata").addEventListener("click", () => {
    if (busy || !isActive() || !confirmation.open || !staged || terminal() || quarantined) return;
    const selected = staged;
    staged = null; confirmation.close();
    if (selected.generation !== generation || selected.baseline !== baseline) return;
    void run(async parentCurrent => {
      // Mark pending before fetch. A failed/lost/late response never re-arms this descriptor.
      attempt = selected.descriptor;
      observe("pending", "metaPending"); controls();
      const current = () => parentCurrent() && generation === selected.generation
        && baseline === selected.baseline && attempt === selected.descriptor && !quarantined && isActive();
      let deniedGeneration = null;
      try {
        const response = await write(selected.descriptor, status => {
          if (!current()) return;
          collectingDenial = true;
          try { sessionRejected({ status, raw: "" }); }
          finally { collectingDenial = false; }
          deniedGeneration = generation;
        });
        if (deniedGeneration !== null) {
          // A complete canonical 401 may refine Session status, never restore the buffer or
          // apply a save. Page lifecycle/discard/new context invalidates this permission too.
          if (parentCurrent() && generation === deniedGeneration && quarantined && attempt === selected.descriptor)
            sessionRejected(response);
          return;
        }
        if (!current()) {
          if (!quarantined && attempt === selected.descriptor) { observe("unknown", "metaUnknown"); controls(); }
          return;
        }
        if (response.status === 200) {
          const receipt = parseMetadataSaved(response.raw, selected.baseline, selected.descriptor);
          if (!current()) return;
          const snapshot = Object.freeze({ ...receipt, contents: selected.baseline.contents });
          baseline = snapshot; draft = copy(receipt.manifest); attempt = null; dirty = false;
          observe("saved", "metaSaved"); renderDraft(); onSaved(snapshot);
        } else {
          observe(response.status === 409 && conflict.test(response.raw) ? "conflict" : "unknown",
            response.status === 409 && conflict.test(response.raw) ? "metaConflict" : "metaUnknown");
          if (response.status === 401 || response.status === 403) sessionRejected(response);
        }
      } catch (error) {
        if (current()) {
          observe("unknown", "metaUnknown");
          if (error?.deniedStatus === 401 || error?.deniedStatus === 403)
            sessionRejected({ status: error.deniedStatus, raw: "" });
        } else if (!quarantined && attempt === selected.descriptor) observe("unknown", "metaUnknown");
      } finally { controls(); }
    });
  });
  window.addEventListener("beforeunload", event => {
    if (held()) { event.preventDefault(); event.returnValue = ""; }
  });
  reset();
  return Object.freeze({ invalidate, leave, hasWork: held,
    acceptSnapshot(value) { if (held()) return; reset(); baseline = value; controls(); },
    setBusy(value) { busy = value; controls(); },
    translate() { state.textContent = label(stateKey); if (!quarantined) renderDraft(); controls(); } });
}
