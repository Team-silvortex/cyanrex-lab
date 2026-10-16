import { useRef } from "react";
import { useI18n } from "../../i18n/context";
import TextPayloadEditor from "../editor/EditorWorkspace";
import { MAX_TASK_PAYLOAD_ITEMS } from "./taskDraft";
import { useTaskDraftController } from "./useTaskDraftController";

/** A local draft container, not a persisted Task or a source of server authority. */
export default function TaskDraftWorkspace() {
  const { t } = useI18n();
  const upload = useRef<HTMLInputElement>(null);
  const textFiles = useRef<HTMLInputElement>(null);
  const { draft, generation, selectedId, selected, dirty, notice, busy, dialog,
    changeTitle, addText, changeItem, removeText, reset, importDraft, importFiles, exportDraft, selectItem,
  } = useTaskDraftController(t);

  return <div className="task-draft-workspace">
    {dialog}
    <header className="page-header"><div><p className="brand-kicker">{t("taskDraft.kicker")}</p>
      <h2>{t("taskDraft.title")}</h2><p className="meta">{t("taskDraft.subtitle")}</p>
    </div></header>
    <p className="task-draft-local-note">{t("taskDraft.localOnly")}</p>
    <section className="panel task-draft-summary">
      <label className="field-label"><span>{t("taskDraft.taskTitle")}</span>
        <input data-testid="task-title" value={draft.title} maxLength={256} disabled={busy}
          placeholder={t("taskDraft.titlePlaceholder")} onChange={event => changeTitle(event.target.value)} />
      </label>
      <div className="task-draft-actions" role="group" aria-label={t("taskDraft.title")}>
        <input data-testid="task-draft-import" ref={upload} hidden type="file" accept=".json,application/json" disabled={busy}
          onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; importDraft(file); }} />
        <button type="button" className="button-secondary" disabled={busy} onClick={() => upload.current?.click()}>{t("taskDraft.import")}</button>
        <button data-testid="task-draft-export" type="button" className="button-primary" disabled={busy} onClick={exportDraft}>{t("taskDraft.export")}</button>
        <button data-testid="task-draft-reset" type="button" className="button-danger" disabled={busy} onClick={reset}>{t("taskDraft.reset")}</button>
      </div>
      <p data-testid="task-draft-status" className="meta task-draft-status">{t(dirty ? "taskDraft.unsaved" : "taskDraft.emptyStatus")}</p>
      {notice && <p className="task-draft-notice" role={notice === "taskDraft.downloadStarted" || notice === "taskDraft.filesAdded" ? "status" : "alert"}>{t(notice)}</p>}
    </section>
    <div className="task-draft-layout">
      <aside className="panel task-payload-panel" aria-label={t("taskDraft.payload")}>
        <div className="task-payload-heading"><h3>{t("taskDraft.payload")}</h3>
          <span className="meta">{t("taskDraft.payloadCount", { count: draft.payload.length, max: MAX_TASK_PAYLOAD_ITEMS })}</span>
        </div>
        {draft.payload.length === 0 && <p className="meta">{t("taskDraft.payloadHint")}</p>}
        <button data-testid="task-payload-add" type="button" className="button-secondary"
          disabled={busy || draft.payload.length >= MAX_TASK_PAYLOAD_ITEMS} onClick={addText}>{t("taskDraft.addText")}</button>
        <input data-testid="task-text-import" ref={textFiles} hidden type="file" multiple
          accept="text/*,.json,.yaml,.yml,.toml,.xml,.rs,.py,.c,.h,.cpp,.hpp,.ts,.js,.md,.sql,.sh"
          disabled={busy || draft.payload.length >= MAX_TASK_PAYLOAD_ITEMS}
          onChange={event => {
            // FileList is live: capture it before clearing so this selection can be retried.
            const files = Array.from(event.currentTarget.files || []);
            event.currentTarget.value = ""; importFiles(files);
          }} />
        <button data-testid="task-text-import-open" type="button" className="button-secondary"
          disabled={busy || draft.payload.length >= MAX_TASK_PAYLOAD_ITEMS}
          onClick={() => textFiles.current?.click()}>{t("taskDraft.importFiles")}</button>
        <ul data-testid="task-payload-list" className="task-payload-list">
          {draft.payload.map(item => <li key={item.id}>
            <button data-testid={`task-payload-item-${item.id}`} data-payload-id={item.id} type="button"
              className={`task-payload-item${item.id === selectedId ? " is-selected" : ""}`} aria-pressed={item.id === selectedId}
              disabled={busy} onClick={() => selectItem(item.id)}>
              <span className="task-payload-filename">{item.filename}</span><span className="meta">{t("taskDraft.textItem")}</span>
            </button>
          </li>)}
        </ul>
        {draft.payload.length === 0 && <p className="meta">{t("taskDraft.noPayload")}</p>}
        <button data-testid="task-payload-remove" type="button" className="button-danger" disabled={busy || !selected}
          onClick={removeText}>{t("taskDraft.remove")}</button>
      </aside>
      <section key={generation} className="task-payload-content" aria-label={t("taskDraft.selectedPayload")}>
        {selected ? <TextPayloadEditor key={selected.id} item={selected} disabled={busy}
          onChange={(expected, patch) => changeItem(selected.id, generation, expected, patch)} />
          : <div className="panel task-payload-empty"><h3>{t("taskDraft.emptyTitle")}</h3><p className="meta">{t("taskDraft.emptyDescription")}</p></div>}
      </section>
    </div>
  </div>;
}
