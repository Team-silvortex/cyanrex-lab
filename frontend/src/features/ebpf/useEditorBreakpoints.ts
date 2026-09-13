import { useCallback, useEffect, useRef, useState } from "react";

import { clampBreakpoints, parseStoredBreakpoints, sameNumberList } from "./editorBreakpointUtils";
import { loadPageState, savePageState } from "../../utils/pageState";

type Args = { code: string; editorRef: { current: any }; monacoRef: { current: any }; hitLine?: number | null };
type Owner = { editor: any; monaco: any; model: any; decorations: string[];
  disposables: Array<{ dispose: () => void }>; modelDisposal?: { dispose: () => void } };
const BREAKPOINT_STORAGE_KEY = "ebpf_debug_breakpoints_v1";

export function useEbpfEditorBreakpoints({ code, editorRef, monacoRef, hitLine }: Args) {
  const [debugBreakpoints, setDebugBreakpoints] = useState<number[]>(() =>
    parseStoredBreakpoints(loadPageState<unknown>(BREAKPOINT_STORAGE_KEY)));
  const ownerRef = useRef<Owner | null>(null);
  const latest = useRef({ code, debugBreakpoints, hitLine }); latest.current = { code, debugBreakpoints, hitLine };
  useEffect(() => { savePageState(BREAKPOINT_STORAGE_KEY, debugBreakpoints); }, [debugBreakpoints]);

  const active = useCallback((owner: Owner) => ownerRef.current === owner && editorRef.current === owner.editor
    && monacoRef.current === owner.monaco, [editorRef, monacoRef]);
  const clearModel = useCallback((owner: Owner) => {
    if (owner.decorations.length && owner.model && !owner.model.isDisposed?.()) {
      // IDs belong to the model they were created on, not the editor's possibly replaced model.
      owner.model.deltaDecorations(owner.decorations, []);
    }
    owner.decorations = [];
  }, []);
  const release = useCallback((owner: Owner | null) => {
    if (!owner) return;
    if (ownerRef.current === owner) ownerRef.current = null;
    clearModel(owner); owner.modelDisposal?.dispose(); owner.modelDisposal = undefined;
    for (const disposable of owner.disposables.splice(0)) disposable.dispose();
  }, [clearModel]);
  const sync = useCallback((owner: Owner) => {
    if (!active(owner) || !owner.model || owner.model.isDisposed?.()) return;
    const count = owner.model.getLineCount();
    setDebugBreakpoints(current => {
      const next = clampBreakpoints(current, count);
      return sameNumberList(current, next) ? current : next;
    });
  }, [active]);
  const decorate = useCallback((owner: Owner) => {
    if (!active(owner) || !owner.model || owner.model !== owner.editor.getModel() || owner.model.isDisposed?.()) return;
    const { code, debugBreakpoints, hitLine } = latest.current;
    const count = owner.model.getLineCount();
    const hit = typeof hitLine === "number" && Number.isSafeInteger(hitLine) && hitLine > 0 && hitLine <= count
      && owner.model.getValue() === code ? hitLine : null;
    const lines = [...new Set([...clampBreakpoints(debugBreakpoints, count), ...(hit === null ? [] : [hit])])];
    owner.decorations = owner.editor.deltaDecorations(owner.decorations, lines.map(line => ({
      range: new owner.monaco.Range(line, 1, line, 1), options: {
        isWholeLine: true, className: line === hit ? "cyanrex-breakpoint-hit-line" : "cyanrex-breakpoint-line",
        glyphMarginClassName: line === hit ? "cyanrex-breakpoint-hit-glyph" : "cyanrex-breakpoint-glyph",
        glyphMarginHoverMessage: { value: line === hit ? "Breakpoint hit" : "Breakpoint" },
      },
    })));
  }, [active]);
  useEffect(() => { if (ownerRef.current) { sync(ownerRef.current); decorate(ownerRef.current); } }, [code, hitLine, debugBreakpoints, sync, decorate]);
  useEffect(() => () => release(ownerRef.current), [release]);

  const onEditorReadyForDebug = useCallback((editor: any, monaco: any) => {
    release(ownerRef.current);
    const owner: Owner = { editor, monaco, model: null, decorations: [], disposables: [] };
    ownerRef.current = owner; editor.updateOptions({ glyphMargin: true });
    const bindModel = () => {
      if (!active(owner)) return;
      clearModel(owner); owner.modelDisposal?.dispose();
      owner.model = editor.getModel();
      const boundModel = owner.model;
      owner.modelDisposal = boundModel?.onWillDispose?.(() => {
        if (!active(owner) || owner.model !== boundModel) return;
        clearModel(owner); owner.model = null;
      });
      sync(owner); decorate(owner);
    };
    const toggle = (line: number) => {
      if (!active(owner) || !owner.model || owner.model !== editor.getModel() || owner.model.isDisposed?.()) return;
      const count = owner.model.getLineCount();
      if (!Number.isSafeInteger(line) || line < 1 || line > count) return;
      setDebugBreakpoints(current => current.includes(line) ? current.filter(value => value !== line)
        : clampBreakpoints([...current, line], count));
    };
    owner.disposables.push(editor.onMouseDown((event: any) => {
      if (!active(owner) || event.target?.type !== monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) return;
      event.event.preventDefault(); toggle(event.target?.position?.lineNumber);
    }), editor.onKeyDown((event: any) => {
      if (!active(owner) || event.keyCode !== monaco.KeyCode.F9) return;
      event.preventDefault(); toggle(editor.getPosition()?.lineNumber);
    }));
    const changed = editor.onDidChangeModel?.(bindModel), disposed = editor.onDidDispose?.(() => release(owner));
    if (changed) owner.disposables.push(changed);
    if (disposed) owner.disposables.push(disposed);
    bindModel();
  }, [active, clearModel, decorate, release, sync]);

  const clearDebugBreakpoints = useCallback(() => setDebugBreakpoints([]), []);
  return { debugBreakpoints, clearDebugBreakpoints, onEditorReadyForDebug };
}
