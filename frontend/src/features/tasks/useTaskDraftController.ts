import { useEffect, useRef, useState } from "react";
import { useConfirmedAction } from "../../components/useConfirmedAction";
import { safeEditorFilename } from "../editor/document";
import { useDraftWarning } from "../editor/useDraftWarning";
import { importTextFiles, preflightTextFiles } from "./textFileImport";
import {
  addPayloadItem, createTaskDraft, createTextPayloadItem, MAX_TASK_DRAFT_BYTES, MAX_TASK_PAYLOAD_ITEMS,
  parseTaskDraft, removePayloadItem, serializeTaskDraft, updatePayloadItem, updateTaskTitle,
  type TaskDraft, type TextPayloadItem,
} from "./taskDraft";

const errorKeys = new Set(["taskDraft.invalid", "taskDraft.stale", "taskDraft.tooLarge", "taskDraft.tooMany", "taskDraft.readFailed"]);
const errorKey = (error: unknown) => error instanceof Error && errorKeys.has(error.message)
  ? error.message : "taskDraft.failed";
const localItemId = () => `text-${Array.from(crypto.getRandomValues(new Uint32Array(4)),
  part => part.toString(16).padStart(8, "0")).join("")}`;

/** Local draft lifecycle only, not persistence, server authority or shared task state. */
export function useTaskDraftController(t: (key: string, vars?: Record<string, string | number>) => string) {
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
  const importFiles = (selection: readonly File[]) => {
    if (!selection.length || !mounted.current || safety.isBusy()) return;
    const files = Object.freeze([...selection]);
    const expected = revision.current, currentDraft = draftRef.current;
    let reviewed;
    try { reviewed = preflightTextFiles(currentDraft, files); }
    catch (error) { setNotice(errorKey(error)); return; }
    safety.request({ action: t("taskDraft.importFiles"), description: t("taskDraft.importFilesConfirm"),
      targets: reviewed.map(file => file.name), dangerous: false,
      details: [
        { label: t("taskDraft.taskTitle"), value: currentDraft.title || t("taskDraft.untitled") },
        { label: t("taskDraft.selectedFiles"), value: String(reviewed.length) },
        { label: t("taskDraft.payload"), value: t("taskDraft.payloadCount", { count: currentDraft.payload.length + reviewed.length, max: MAX_TASK_PAYLOAD_ITEMS }) },
        { label: t("taskDraft.fileSize"), value: t("taskDraft.bytes", { count: reviewed.reduce((sum, file) => sum + file.size, 0) }) },
      ],
    }, async signal => {
      checkReviewed(expected, signal);
      let next: TaskDraft;
      try { next = await importTextFiles(currentDraft, files, localItemId, signal, () => checkReviewed(expected, signal)); }
      catch (error) {
        const key = errorKey(error);
        throw new Error(t(key === "taskDraft.readFailed" ? "taskDraft.importReadFailed" : key));
      }
      checkReviewed(expected, signal);
      commit(next);
      if (!selectedRef.current) choose(next.payload[currentDraft.payload.length]?.id ?? null);
      setNotice("taskDraft.filesAdded");
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
  const selectItem = (id: string | null) => {
    if (mounted.current && !safety.isBusy()) choose(id);
  };

  return {
    draft, generation, selectedId, selected, dirty, notice, busy: safety.busy, dialog: safety.dialog,
    changeTitle, addText, changeItem, removeText, reset, importDraft, importFiles, exportDraft, selectItem,
  };
}
