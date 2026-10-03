import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/context";
import { useConfirmedAction } from "../../components/useConfirmedAction";
import TextPayloadEditor from "../editor/EditorWorkspace";
import { safeEditorFilename } from "../editor/document";
import { useDraftWarning } from "../editor/useDraftWarning";
import {
  addPayloadItem, createTaskDraft, createTextPayloadItem, MAX_TASK_DRAFT_BYTES, MAX_TASK_PAYLOAD_ITEMS,
  parseTaskDraft, removePayloadItem, serializeTaskDraft, updatePayloadItem, updateTaskTitle,
  type TaskDraft, type TextPayloadItem,
} from "./taskDraft";

const errorKeys = new Set(["taskDraft.invalid", "taskDraft.stale", "taskDraft.tooLarge", "taskDraft.tooMany"]);
const errorKey = (error: unknown) => error instanceof Error && errorKeys.has(error.message)
  ? error.message : "taskDraft.failed";
const localItemId = () => `text-${Array.from(crypto.getRandomValues(new Uint32Array(4)),
  part => part.toString(16).padStart(8, "0")).join("")}`;

/** A local draft container, not a persisted Task or a source of server authority. */
export default function TaskDraftWorkspace() {
  const { t } = useI18n();
  const [draft, setDraft] = useState<TaskDraft>(createTaskDraft);
  const draftRef = useRef(draft);
  const revision = useRef(0);
  const generationRef = useRef(0);
  const [generation, setGeneration] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedRef = useRef<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [notice, setNotice] = useState("");
  const mounted = useRef(true);
  const upload = useRef<HTMLInputElement>(null);
  const safety = useConfirmedAction();
  const selected = draft.payload.find(item => item.id === selectedId);
  useDraftWarning(dirty, t("taskDraft.leaveConfirm"));
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; revision.current++; };
  }, []);

  const choose = (id: string | null) => { selectedRef.current = id; setSelectedId(id); };
  const commit = (next: TaskDraft, replace = false, empty = false) => {
    // Update authority refs before scheduling renders, including title-only edits.
    draftRef.current = next;
    revision.current++;
    if (replace) {
      generationRef.current++;
      setGeneration(generationRef.current);
      choose(next.payload[0]?.id ?? null);
    }
    setDraft(next); setDirty(!empty); setNotice("");
  };
  const checkReviewed = (expected: number, signal: AbortSignal) => {
    if (!mounted.current || signal.aborted || revision.current !== expected) throw new Error(t("taskDraft.stale"));
  };
  const changeTitle = (title: string) => {
    if (!mounted.current || safety.isBusy()) return;
    try { commit(updateTaskTitle(draftRef.current, title)); }
    catch (error) { setNotice(errorKey(error)); }
  };
  const addText = () => {
    if (!mounted.current || safety.isBusy()) return;
    try {
      const item = createTextPayloadItem(localItemId());
      commit(addPayloadItem(draftRef.current, item)); choose(item.id);
    } catch (error) { setNotice(errorKey(error)); }
  };
  const changeItem = (id: string, ownerGeneration: number, expectedRevision: number,
    patch: Partial<Pick<TextPayloadItem, "filename" | "language" | "text">>) => {
    if (!mounted.current || safety.isBusy() || generationRef.current !== ownerGeneration || selectedRef.current !== id) {
      throw new Error("taskDraft.stale");
    }
    try { commit(updatePayloadItem(draftRef.current, id, expectedRevision, patch)); }
    catch (error) {
      const key = errorKey(error); setNotice(key);
      // The controlled editor must restore its accepted value and surface the failure.
      throw new Error(key);
    }
  };
  const removeText = () => {
    if (!mounted.current || safety.isBusy()) return;
    const target = draftRef.current.payload.find(item => item.id === selectedRef.current);
    if (!target) return;
    const expected = revision.current;
    safety.request({ action: t("taskDraft.remove"), description: t("taskDraft.removeConfirm"),
      targets: [target.filename], details: [{ label: t("taskDraft.taskTitle"), value: draftRef.current.title || t("taskDraft.untitled") }],
    }, signal => {
      checkReviewed(expected, signal);
      let next: TaskDraft;
      try { next = removePayloadItem(draftRef.current, target.id, target.revision); }
      catch (error) { throw new Error(t(errorKey(error))); }
      commit(next); choose(next.payload[0]?.id ?? null);
    });
  };
  const reset = () => {
    if (!mounted.current || safety.isBusy()) return;
    const expected = revision.current;
    safety.request({ action: t("taskDraft.reset"), description: t("taskDraft.resetConfirm"),
      targets: [draftRef.current.title || t("taskDraft.untitled")],
      details: [{ label: t("taskDraft.payload"), value: t("taskDraft.payloadCount", { count: draftRef.current.payload.length, max: MAX_TASK_PAYLOAD_ITEMS }) }],
    }, signal => { checkReviewed(expected, signal); commit(createTaskDraft(), true, true); });
  };
  const importDraft = (file: File | undefined) => {
    if (!file || !mounted.current || safety.isBusy()) return;
    if (file.size > MAX_TASK_DRAFT_BYTES) { setNotice("taskDraft.tooLarge"); return; }
    const expected = revision.current;
    safety.request({ action: t("taskDraft.import"), description: t("taskDraft.importConfirm"),
      targets: [draftRef.current.title || t("taskDraft.untitled"), safeEditorFilename(file.name)],
      details: [{ label: t("taskDraft.fileSize"), value: t("taskDraft.bytes", { count: file.size }) }],
    }, async signal => {
      checkReviewed(expected, signal);
      let bytes: Uint8Array;
      try { bytes = new Uint8Array(await file.arrayBuffer()); }
      catch { throw new Error(t("taskDraft.readFailed")); }
      checkReviewed(expected, signal);
      let next: TaskDraft;
      try { next = parseTaskDraft(bytes); }
      catch (error) { throw new Error(t(errorKey(error))); }
      checkReviewed(expected, signal);
      commit(next, true);
    });
  };
  const exportDraft = () => {
    if (!mounted.current || safety.isBusy()) return;
    try {
      // Serialization validates the expanded JSON size before creating a download.
      const text = serializeTaskDraft(draftRef.current);
      const url = URL.createObjectURL(new Blob([text], { type: "application/json;charset=utf-8" }));
      try {
        const anchor = document.createElement("a");
        anchor.href = url; anchor.download = "cyanrex-task-draft.json"; anchor.hidden = true;
        document.body.appendChild(anchor);
        try { anchor.click(); } finally { anchor.remove(); }
      } finally { setTimeout(() => URL.revokeObjectURL(url), 1000); }
      setNotice("taskDraft.downloadStarted");
    } catch (error) { setNotice(errorKey(error)); }
  };

  return <div className="task-draft-workspace">
    {safety.dialog}
    <header className="page-header"><div><p className="brand-kicker">{t("taskDraft.kicker")}</p>
      <h2>{t("taskDraft.title")}</h2><p className="meta">{t("taskDraft.subtitle")}</p>
    </div></header>
    <p className="task-draft-local-note">{t("taskDraft.localOnly")}</p>
    <section className="panel task-draft-summary">
      <label className="field-label"><span>{t("taskDraft.taskTitle")}</span>
        <input data-testid="task-title" value={draft.title} maxLength={256} disabled={safety.busy}
          placeholder={t("taskDraft.titlePlaceholder")} onChange={event => changeTitle(event.target.value)} />
      </label>
      <div className="task-draft-actions">
        <input data-testid="task-draft-import" ref={upload} hidden type="file" accept=".json,application/json" disabled={safety.busy}
          onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; importDraft(file); }} />
        <button type="button" className="button-secondary" disabled={safety.busy} onClick={() => upload.current?.click()}>{t("taskDraft.import")}</button>
        <button data-testid="task-draft-export" type="button" className="button-primary" disabled={safety.busy} onClick={exportDraft}>{t("taskDraft.export")}</button>
        <button data-testid="task-draft-reset" type="button" className="button-secondary" disabled={safety.busy} onClick={reset}>{t("taskDraft.reset")}</button>
      </div>
      <p data-testid="task-draft-status" className="meta task-draft-status">{t(dirty ? "taskDraft.unsaved" : "taskDraft.emptyStatus")}</p>
      {notice && <p className="task-draft-notice" role={notice === "taskDraft.downloadStarted" ? "status" : "alert"}>{t(notice)}</p>}
    </section>
    <div className="task-draft-layout">
      <aside className="panel task-payload-panel" aria-label={t("taskDraft.payload")}>
        <div className="task-payload-heading"><h3>{t("taskDraft.payload")}</h3>
          <span className="meta">{t("taskDraft.payloadCount", { count: draft.payload.length, max: MAX_TASK_PAYLOAD_ITEMS })}</span>
        </div>
        <p className="meta">{t("taskDraft.payloadHint")}</p>
        <button data-testid="task-payload-add" type="button" className="button-secondary"
          disabled={safety.busy || draft.payload.length >= MAX_TASK_PAYLOAD_ITEMS} onClick={addText}>{t("taskDraft.addText")}</button>
        <ul data-testid="task-payload-list" className="task-payload-list">
          {draft.payload.map(item => <li key={item.id}>
            <button data-testid={`task-payload-item-${item.id}`} data-payload-id={item.id} type="button"
              className={`task-payload-item${item.id === selectedId ? " is-selected" : ""}`} aria-pressed={item.id === selectedId}
              disabled={safety.busy} onClick={() => { if (mounted.current && !safety.isBusy()) choose(item.id); }}>
              <span className="task-payload-filename">{item.filename}</span><span className="meta">{t("taskDraft.textItem")}</span>
            </button>
          </li>)}
        </ul>
        {draft.payload.length === 0 && <p className="meta">{t("taskDraft.noPayload")}</p>}
        <button data-testid="task-payload-remove" type="button" className="button-danger" disabled={safety.busy || !selected}
          onClick={removeText}>{t("taskDraft.remove")}</button>
      </aside>
      <section key={generation} className="task-payload-content" aria-label={t("taskDraft.selectedPayload")}>
        {selected ? <TextPayloadEditor key={selected.id} item={selected} disabled={safety.busy}
          onChange={(expected, patch) => changeItem(selected.id, generation, expected, patch)} />
          : <div className="panel task-payload-empty"><h3>{t("taskDraft.emptyTitle")}</h3><p className="meta">{t("taskDraft.emptyDescription")}</p></div>}
      </section>
    </div>
  </div>;
}
