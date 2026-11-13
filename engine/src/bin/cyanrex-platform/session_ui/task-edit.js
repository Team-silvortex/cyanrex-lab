import { prepareMetadataSave, parseMetadataSaved } from './task-contract.js';
import { platformRequest } from './request.js';
import { prepareBodySave, decodeSaveReceipt, editTextareaText } from './body-contract.js';

// One page-memory editor. Only the trusted host's explicit capability enables fixed-slot body saves.
export function createMetadataEditor({ run, isActive, sessionRejected, label, onSaved }) {
  const byId = id => document.getElementById(id);
  const start = byId("edit-task"), panel = byId("task-metadata-editor"), state = byId("task-edit-state");
  const title = byId("task-edit-title"), item = byId("task-edit-item");
  const filename = byId("task-edit-filename"), language = byId("task-edit-language");
  const save = byId("save-task-metadata"), discard = byId("discard-task-edit");
  const confirmation = byId("save-metadata-dialog"), leaving = byId("task-leave-dialog");
  const text = byId("task-edit-text"), progress = byId("task-save-progress");
  const bodyEnabled = document.body.dataset.taskTextPublication === "enabled";
  let texts = null, bodyProgress = null, reviewFocus = false;
  let baseline = null, draft = null, attempt = null, staged = null, departure = null;
  let busy = true, editing = false, dirty = false, quarantined = false, generation = 0;
  let phase = "clean", stateKey = "metaClean", controller = null, collectingDenial = false;
  const held = () => dirty || attempt !== null || quarantined;
  const terminal = () => attempt !== null;
  const copy = value => ({ title: value.title,
    items: value.payload.map(binding => ({ filename: binding.filename, language: binding.language })) });
  const textChanged = () => bodyEnabled && Boolean(texts?.some((value, index) => value !== baseline?.contents[index]?.text));
  const bodyGuard = () => Object.freeze({ generation, revision: baseline.task.revision, contextEpoch: generation });
  function localLabel(id, key) {
    const node = byId(id); node.dataset.i18n = key; node.textContent = label(key);
  }
  function showProgress() {
    progress.hidden = !bodyProgress || quarantined;
    progress.dataset.confirmedArtifacts = String(bodyProgress?.confirmedArtifacts || 0);
    progress.dataset.taskConfirmed = String(Boolean(bodyProgress?.taskConfirmed));
    progress.textContent = bodyProgress && !quarantined ? label("bodyProgress")
      .replace("{count}", String(bodyProgress.confirmedArtifacts))
      .replace("{task}", label(bodyProgress.taskConfirmed ? "bodyTaskConfirmed" : "bodyTaskUnconfirmed")) : "";
  }

  function observe(value, key) {
    if (bodyEnabled) key = ({ metaClean: "bodyClean", metaDirty: "bodyDirty", metaInvalid: "bodyInvalid" })[key] || key;
    phase = value; stateKey = key;
    state.dataset.state = value; state.dataset.i18n = key;
    state.textContent = label(key);
  }
  function controls() {
    if (bodyEnabled) {
      localLabel("edit-task", "bodyStart"); localLabel("save-task-metadata", "bodySave");
      localLabel("tasks-text-boundary", "bodyBoundary");
      localLabel("platform-footer", "bodyFooter");
    }
    byId("task-text-editor").hidden = !bodyEnabled || !editing || quarantined;
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
    text.disabled = !bodyEnabled || busy || !isActive() || quarantined || !draft?.items.length;
    text.readOnly = terminal();
    save.disabled = busy || !isActive() || quarantined || !editing || !dirty || terminal();
    for (const id of ["confirm-save-metadata", "cancel-save-metadata", "confirm-task-leave", "cancel-task-leave"])
      byId(id).disabled = busy;
    showProgress();
    if (!busy && reviewFocus && confirmation.open) { reviewFocus = false; byId("cancel-save-metadata").focus(); }
  }
  function emptyDom() {
    title.value = ""; item.replaceChildren(); filename.value = ""; language.value = "";
    text.value = ""; bodyProgress = null; showProgress();
    byId("metadata-save-target").textContent = "";
    byId("metadata-save-summary").textContent = "";
    byId("task-leave-target").textContent = "";
  }
  function closeDialogs() {
    if (confirmation.open) confirmation.close();
    if (leaving.open) leaving.close();
    staged = null; departure = null; reviewFocus = false;
  }
  function reset(keepBaseline = false) {
    generation++;
    controller?.abort(); controller = null;
    attempt?.stop?.("cancelled");
    closeDialogs(); emptyDom();
    if (!keepBaseline) baseline = null;
    draft = null; texts = null; attempt = null; editing = false; dirty = false; quarantined = false;
    observe("clean", "metaClean"); controls();
  }
  function invalidate() {
    if (!held()) { reset(); return; }
    generation++;
    attempt?.stop?.("stale");
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
      text.value = texts?.[item.selectedIndex] || "";
    } else { filename.value = ""; language.value = ""; text.value = ""; }
    controls();
  }
  function inputChanged() {
    if (!baseline || !draft || quarantined || busy || terminal() || !editing) return;
    draft.title = title.value;
    const selected = draft.items[item.selectedIndex];
    if (selected) { selected.filename = filename.value; selected.language = language.value; }
    generation++;
    dirty = JSON.stringify(draft) !== JSON.stringify(copy(baseline.manifest)) || textChanged();
    bodyProgress = null;
    if (!terminal()) observe(dirty ? "dirty" : "clean", dirty ? "metaDirty" : "metaClean");
    controls();
  }
  for (const field of [title, filename, language]) field.addEventListener("input", inputChanged);
  text.addEventListener("input", () => {
    if (!bodyEnabled || busy || terminal() || quarantined || !editing || !texts || item.selectedIndex < 0) return;
    texts[item.selectedIndex] = editTextareaText(texts[item.selectedIndex], text.value);
    inputChanged();
  });
  item.addEventListener("change", () => {
    if (!draft || quarantined || busy) return;
    const selected = draft.items[item.selectedIndex];
    filename.value = selected?.filename || ""; language.value = selected?.language || "";
    text.value = texts?.[item.selectedIndex] || "";
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
    byId("task-leave-warning").textContent = label(terminal() || quarantined ? "metaLeaveUnknown"
      : bodyEnabled ? "bodyLeaveDirty" : "metaLeaveDirty");
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
    texts = bodyEnabled ? baseline.contents.map(item => item.text) : null;
    observe("clean", "metaClean"); renderDraft(); title.focus();
  });

  const ws = "[\\x20\\x09\\x0a\\x0d]*";
  const field = (key, value) => `"${key}"${ws}:${ws}${value}`;
  const object = (first, second) => `\\{${ws}(?:${first}${ws},${ws}${second}|${second}${ws},${ws}${first})${ws}\\}`;
  const conflict = new RegExp(`^${ws}${object(field("schema_version", "1"), field("error",
    object(field("code", '"(?:conflict|stale_revision|invalid_transition)"'), field("outcome", '"unconfirmed"'))))}${ws}$`);

  async function write(descriptor, onDenial) {
    const own = new AbortController(); controller = own;
    const publishing = descriptor.kind === "publish";
    try {
      return await platformRequest(`/platform/v1/tasks/${descriptor.task_id}/${publishing ? "text-artifacts" : "content"}`, publishing ? "POST" : "PUT", {
        signal: own.signal, body: JSON.stringify(descriptor.body), onDenied: onDenial,
      });
    } finally {
      if (controller === own) controller = null;
    }
  }

  function reviewBody() {
    if (!bodyEnabled || !textChanged()) return;
    const selected = { baseline, generation, guard: bodyGuard(),
      edits: Object.freeze({ title: draft.title, items: Object.freeze(draft.items.map((binding, index) =>
        Object.freeze({ ...binding, text: texts[index] }))) }) };
    void run(async parentCurrent => {
      const current = () => parentCurrent() && generation === selected.generation
        && baseline === selected.baseline && isActive() && !quarantined;
      try {
        const allocations = selected.edits.items.map(() => ({ artifact_id: crypto.randomUUID(), revision_id: crypto.randomUUID() }));
        const plan = await prepareBodySave(selected.baseline, selected.edits, selected.guard, allocations);
        if (!current()) { plan.stop("stale"); return; }
        staged = { ...selected, plan };
        localLabel("metadata-save-title", "bodyConfirmTitle");
        localLabel("task-save-description", "bodyConfirmDescription");
        byId("metadata-save-target").textContent = target();
        byId("metadata-save-summary").textContent = `${label("metaTitle")}: ${selected.edits.title}\n`
          + selected.edits.items.map((binding, index) => `${index + 1}. ${binding.filename} · ${binding.language} · ${new TextEncoder().encode(binding.text).length} B`).join("\n");
        reviewFocus = true; confirmation.showModal();
      } catch { if (current()) { observe("invalid-local", "bodyInvalid"); controls(); } }
    });
  }

  async function saveBody(selected, parentCurrent) {
    const plan = selected.plan;
    attempt = plan; bodyProgress = plan.progress();
    observe("pending", "bodyPending"); controls();
    const current = () => parentCurrent() && generation === selected.generation && baseline === selected.baseline
      && attempt === plan && !quarantined && isActive();
    try {
      while (current()) {
        const step = plan.next(selected.guard);
        if (!step) throw new Error("taskSave.invalid");
        bodyProgress = plan.progress(); showProgress();
        let deniedGeneration = null;
        const response = await write(step, status => {
          if (!current()) return;
          collectingDenial = true;
          try { sessionRejected({ status, raw: "" }); }
          finally { collectingDenial = false; }
          deniedGeneration = generation;
        });
        if (deniedGeneration !== null) {
          if (parentCurrent() && generation === deniedGeneration && quarantined && attempt === plan)
            sessionRejected(response);
          return;
        }
        if (!current()) { plan.stop("stale"); return; }
        if (response.status !== (step.kind === "publish" ? 201 : 200)) {
          const conflicted = response.status === 409 && conflict.test(response.raw);
          plan.stop(conflicted ? "conflict" : "unconfirmed");
          bodyProgress = plan.progress();
          observe(conflicted ? "conflict" : "unknown", conflicted ? "metaConflict" : "metaUnknown");
          return;
        }
        const receipt = decodeSaveReceipt(response.raw);
        if (!plan.confirm(step, receipt, selected.guard)) {
          bodyProgress = plan.progress(); observe("unknown", "metaUnknown"); return;
        }
        bodyProgress = plan.progress(); showProgress();
        if (step.kind === "replace" && bodyProgress.taskConfirmed) {
          const snapshot = Object.freeze({ ...receipt, contents: Object.freeze(selected.edits.items.map((binding, index) =>
            Object.freeze({ artifact: receipt.manifest.payload[index].artifact, text: binding.text }))) });
          baseline = snapshot; draft = copy(snapshot.manifest);
          texts = snapshot.contents.map(binding => binding.text); attempt = null; dirty = false;
          observe("saved", "bodySaved"); renderDraft(); onSaved(snapshot, true); return;
        }
      }
      plan.stop("stale");
    } catch (error) {
      plan.stop("unconfirmed");
      if (current()) {
        bodyProgress = plan.progress(); observe("unknown", "metaUnknown");
        if (error?.deniedStatus === 401 || error?.deniedStatus === 403)
          sessionRejected({ status: error.deniedStatus, raw: "" });
      }
    } finally { controls(); }
  }

  save.addEventListener("click", () => {
    if (busy || !isActive() || !editing || !dirty || terminal() || quarantined || confirmation.open) return;
    if (textChanged()) { reviewBody(); return; }
    try {
      const descriptor = prepareMetadataSave(baseline, draft);
      staged = { descriptor, generation, baseline };
      localLabel("metadata-save-title", "metaConfirmTitle");
      localLabel("task-save-description", "metaConfirmDescription");
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
    if (selected.plan) { void run(parentCurrent => saveBody(selected, parentCurrent)); return; }
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
          texts = bodyEnabled ? snapshot.contents.map(binding => binding.text) : null;
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
