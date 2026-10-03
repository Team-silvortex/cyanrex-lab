import { useCallback, useEffect, useRef, useState } from "react";
import type * as Monaco from "monaco-editor";
import { useI18n } from "../../i18n/context";
import { useConfirmedAction } from "../../components/useConfirmedAction";
import { BUILTIN_LANGUAGES, detectEditorLanguage, getEditorLanguage } from "./languages";
import { decodeEditorBytes, MAX_EDITOR_IMPORT_BYTES, safeEditorFilename } from "./document";
import { useEditorSession, type EditorSession } from "./useEditorSession";

export type TextEditorDocument = {
  id: string; revision: number; filename: string; language: string; text: string;
};
type TextEditorPatch = Partial<Pick<TextEditorDocument, "filename" | "language" | "text">>;
type Props = {
  item: TextEditorDocument;
  onChange: (expectedRevision: number, patch: TextEditorPatch) => void;
  disabled?: boolean;
};

/** A content editor, not a content store. The parent owns the document and accepts each revision. */
export default function EditorWorkspace({ item, onChange, disabled = false }: Props) {
  const { t } = useI18n();
  const container = useRef<HTMLDivElement>(null);
  const upload = useRef<HTMLInputElement>(null);
  const mounted = useRef(true);
  const live = useRef({ item, onChange, disabled });
  live.current = { item, onChange, disabled };
  const editorSession = useRef<EditorSession | null>(null);
  const [problems, setProblems] = useState<Monaco.editor.IMarker[]>([]);
  const [notice, setNotice] = useState("");
  const [working, setWorking] = useState(false);
  const safety = useConfirmedAction();
  const commit = (patch: TextEditorPatch, expected = live.current.item.revision) => {
    const current = live.current;
    if (!mounted.current || current.disabled || current.item.revision !== expected) throw new Error("taskDraft.stale");
    current.onChange(expected, patch);
    // Monaco can dispatch multiple edits before React renders the parent's accepted revision.
    live.current = { ...current, item: { ...current.item, ...patch, revision: expected + 1 } };
    setNotice("");
  };
  const commitText = (patch: TextEditorPatch) => {
    if (safety.isBusy()) throw new Error("taskDraft.stale");
    commit(patch);
  };
  const commitRef = useRef(commitText); commitRef.current = commitText;
  const changed = useCallback((text: string) => {
    try { commitRef.current({ text }); }
    catch (error) {
      setNotice(error instanceof Error ? error.message : "taskDraft.invalid");
      const currentSession = editorSession.current;
      const model = currentSession?.editor.getModel();
      queueMicrotask(() => {
        if (mounted.current && currentSession === editorSession.current && currentSession?.editor.getModel() === model) {
          currentSession?.synchronize(live.current.item.language, live.current.item.text);
        }
      });
    }
  }, []);
  const { session, failed } = useEditorSession(container, item, changed, setProblems);
  editorSession.current = session;
  const busy = disabled || safety.busy || working || !session;
  const { filename, language } = item;
  const profile = getEditorLanguage(language)!;
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => { session?.editor.updateOptions({ readOnly: disabled || safety.busy }); }, [session, disabled, safety.busy]);
  useEffect(() => { session?.synchronize(item.language, item.text); }, [session, item.language, item.text]);
  const update = (patch: TextEditorPatch) => {
    if (busy) return;
    try { commit(patch); }
    catch (error) { setNotice(error instanceof Error ? error.message : "taskDraft.invalid"); }
  };

  const changeLanguage = (id: string) => {
    const next = getEditorLanguage(id);
    if (!next || !session || busy) return;
    update({ language: id, ...(/^untitled\.[a-z]+$/.test(filename) ? { filename: `untitled${next.extension}` } : {}) });
  };
  const importFile = (file: File | undefined) => {
    if (!file || !session || disabled || safety.isBusy() || working) return;
    if (file.size > MAX_EDITOR_IMPORT_BYTES) { setNotice("editor.tooLarge"); return; }
    const expected = live.current.item.revision;
    const model = session.editor.getModel();
    safety.request({ action: t("editor.import"), description: t("editor.replaceConfirm"),
      targets: [filename, safeEditorFilename(file.name)],
      details: [{ label: t("editor.fileSize"), value: `${file.size} B` }],
    }, async signal => {
      let source: string;
      try { source = decodeEditorBytes(new Uint8Array(await file.arrayBuffer())); }
      catch (error) { throw new Error(t(error instanceof Error && error.message.startsWith("editor.") ? error.message : "editor.importFailed")); }
      if (signal.aborted || !mounted.current) return;
      if (live.current.item.revision !== expected || session.editor.getModel() !== model) throw new Error(t("editor.draftChanged"));
      const next = detectEditorLanguage(file.name);
      try { commit({ language: next.id, text: source, filename: safeEditorFilename(file.name) }, expected); }
      catch (error) { throw new Error(t(error instanceof Error ? error.message : "taskDraft.invalid")); }
    });
  };
  const reset = () => {
    if (!session || disabled || safety.isBusy() || working) return;
    const expected = live.current.item.revision;
    safety.request({ action: t("editor.reset"), description: t("editor.resetConfirm"), targets: [filename] }, () => {
      try { commit({ text: "" }, expected); }
      catch (error) { throw new Error(t(error instanceof Error ? error.message : "taskDraft.invalid")); }
    });
  };
  const download = () => {
    const model = session?.editor.getModel();
    if (!model || busy) return;
    const url = URL.createObjectURL(new Blob([live.current.item.text], { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url; anchor.download = safeEditorFilename(filename); anchor.hidden = true;
    document.body.appendChild(anchor); anchor.click(); anchor.remove();
    // The browser owns the download; requesting it is not proof of a successful disk write.
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    setNotice("editor.downloadStarted");
  };
  const runAction = async (id: string) => {
    if (!session || busy) return;
    session.editor.focus();
    const action = session.editor.getAction(id);
    if (!action || !action.isSupported()) { setNotice("editor.actionUnavailable"); return; }
    setWorking(true);
    try { await action.run(); }
    catch { if (mounted.current) setNotice("editor.actionUnavailable"); }
    finally { if (mounted.current) setWorking(false); }
  };

  return <div className="language-workspace">
    {safety.dialog}
    <section className="panel language-editor-panel">
      <div className="language-editor-toolbar">
        <label className="field-label"><span>{t("editor.filename")}</span>
          <input data-testid="editor-filename" value={filename} maxLength={128} disabled={busy}
            onChange={event => update({ filename: event.target.value })} />
        </label>
        <label className="field-label"><span>{t("editor.language")}</span>
          <select data-testid="language-select" value={language} disabled={busy} onChange={event => changeLanguage(event.target.value)}>
            {BUILTIN_LANGUAGES.map(item => <option key={item.id} value={item.id}>{item.label}</option>)}
          </select>
        </label>
        <div className="language-editor-buttons">
          <input data-testid="editor-import" ref={upload} type="file" hidden onChange={event => {
            const file = event.target.files?.[0]; event.target.value = ""; importFile(file);
          }} />
          <button className="button-secondary" disabled={busy} onClick={() => upload.current?.click()}>{t("editor.import")}</button>
          <button data-testid="editor-download" className="button-primary" disabled={busy} onClick={download}>{t("editor.download")}</button>
          <button data-testid="editor-reset" className="button-secondary" disabled={busy} onClick={reset}>{t("editor.reset")}</button>
        </div>
      </div>
      <div className="language-editor-tools">
        <button data-testid="editor-completion" className="button-secondary" disabled={busy || !profile.capabilities.includes("completion")}
          onClick={() => void runAction("editor.action.triggerSuggest")}>{t("editor.completion")}</button>
        <button data-testid="editor-format" className="button-secondary" disabled={busy || !profile.capabilities.includes("format")}
          onClick={() => void runAction("editor.action.formatDocument")}>{t("editor.format")}</button>
        <button data-testid="editor-symbols" className="button-secondary" disabled={busy || !profile.capabilities.includes("symbols")}
          onClick={() => void runAction("editor.action.quickOutline")}>{t("editor.symbols")}</button>
        <span className="meta">{t("editor.unsaved")}</span>
      </div>
      <div data-testid="editor-capabilities" className="editor-capabilities">
        <strong>{t(`editor.tier.${profile.tier}`)}</strong>
        <span>{profile.capabilities.map(capability => t(`editor.capability.${capability}`)).join(" · ") || t("editor.noService")}</span>
      </div>
      {!session && <p role={failed ? "alert" : "status"}>{t(failed ? "editor.loadFailed" : "editor.loading")}</p>}
      <div ref={container} className="language-editor-surface" />
      {notice && <p role={notice === "editor.downloadStarted" ? "status" : "alert"} className="editor-notice">{t(notice)}</p>}
    </section>
    <section className="panel editor-problems" data-testid="editor-problems" aria-label={t("editor.problems")}>
      <h3>{t("editor.problems")} {profile.capabilities.includes("diagnostics") && `(${problems.length})`}</h3>
      {!profile.capabilities.includes("diagnostics") ? <p className="meta">{t("editor.noDiagnostics")}</p>
        : problems.length === 0 ? <p className="meta">{t("editor.noProblems")}</p>
          : <ul>{problems.map((marker, index) => <li key={`${marker.startLineNumber}:${marker.startColumn}:${index}`}>
            <button type="button" className="button-secondary" disabled={busy} onClick={() => {
              session?.editor.setPosition({ lineNumber: marker.startLineNumber, column: marker.startColumn });
              session?.editor.revealLineInCenter(marker.startLineNumber); session?.editor.focus();
            }}>{marker.startLineNumber}:{marker.startColumn} · {marker.message}</button>
          </li>)}</ul>}
    </section>
    <p className="meta">{t("editor.serviceLimits")}</p>
  </div>;
}
