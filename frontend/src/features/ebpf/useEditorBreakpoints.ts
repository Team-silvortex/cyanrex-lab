import { useCallback, useEffect, useRef, useState } from "react";

import { clampBreakpoints, parseStoredBreakpoints, sameNumberList } from "./editorBreakpointUtils";
import { loadPageState, savePageState } from "../../utils/pageState";

type DebugEditorRefs = {
  editorRef: { current: any };
  monacoRef: { current: any };
};

type UseEbpfEditorBreakpointsArgs = {
  code: string;
  editorRef: DebugEditorRefs["editorRef"];
  monacoRef: DebugEditorRefs["monacoRef"];
  hitLine?: number | null;
};

const BREAKPOINT_STORAGE_KEY = "ebpf_debug_breakpoints_v1";

export function useEbpfEditorBreakpoints({
  code,
  editorRef,
  monacoRef,
  hitLine,
}: UseEbpfEditorBreakpointsArgs) {
  const [debugBreakpoints, setDebugBreakpoints] = useState<number[]>(() =>
    parseStoredBreakpoints(loadPageState<unknown>(BREAKPOINT_STORAGE_KEY)),
  );
  const editorDisposablesRef = useRef<Array<{ dispose: () => void }>>([]);
  const decorationIdsRef = useRef<string[]>([]);

  useEffect(() => {
    savePageState(BREAKPOINT_STORAGE_KEY, debugBreakpoints);
  }, [debugBreakpoints]);

  const syncBreakpointsToModel = useCallback(() => {
    if (!editorRef.current) {
      return;
    }
    const model = editorRef.current.getModel();
    if (!model) {
      return;
    }
    setDebugBreakpoints((current) => {
      const next = clampBreakpoints(current, model.getLineCount());
      return sameNumberList(current, next) ? current : next;
    });
  }, [editorRef]);

  useEffect(() => {
    syncBreakpointsToModel();
  }, [code, syncBreakpointsToModel]);

  const applyBreakpointDecorations = useCallback(() => {
    if (!editorRef.current || !monacoRef.current) {
      return;
    }
    const monaco = monacoRef.current;
    const model = editorRef.current.getModel();
    if (!model) {
      return;
    }

    const decorationLines = Array.from(new Set([
      ...debugBreakpoints,
      ...(typeof hitLine === "number" ? [hitLine] : []),
    ]));
    const decorations = decorationLines.map((line) => ({
      range: new monaco.Range(line, 1, line, 1),
      options: {
        isWholeLine: true,
        className: line === hitLine ? "cyanrex-breakpoint-hit-line" : "cyanrex-breakpoint-line",
        glyphMarginClassName: line === hitLine
          ? "cyanrex-breakpoint-hit-glyph"
          : "cyanrex-breakpoint-glyph",
        glyphMarginHoverMessage: { value: line === hitLine ? "Breakpoint hit" : "Breakpoint" },
      },
    }));

    decorationIdsRef.current = editorRef.current.deltaDecorations(
      decorationIdsRef.current,
      decorations,
    );
  }, [debugBreakpoints, editorRef, hitLine, monacoRef]);

  useEffect(() => {
    applyBreakpointDecorations();
  }, [applyBreakpointDecorations]);

  const clearDisposables = useCallback(() => {
    if (editorRef.current && decorationIdsRef.current.length > 0) {
      editorRef.current.deltaDecorations(decorationIdsRef.current, []);
      decorationIdsRef.current = [];
    }
    for (const item of editorDisposablesRef.current) {
      item.dispose();
    }
    editorDisposablesRef.current = [];
  }, [editorRef]);

  useEffect(() => {
    return () => {
      clearDisposables();
    };
  }, [clearDisposables]);

  const toggleDebugBreakpoint = useCallback(
    (line: number) => {
      if (!editorRef.current) {
        return;
      }
      const model = editorRef.current.getModel();
      const lineCount = model?.getLineCount() ?? 0;
      if (!Number.isInteger(line) || line < 1 || (lineCount > 0 && line > lineCount)) {
        return;
      }

      setDebugBreakpoints((current) => {
        if (current.includes(line)) {
          return current.filter((value) => value !== line);
        }
        return clampBreakpoints([...current, line], lineCount || line);
      });
    },
    [editorRef],
  );

  const clearDebugBreakpoints = useCallback(() => {
    setDebugBreakpoints([]);
  }, []);

  const onEditorReadyForDebug = useCallback(
    (editor: any, monaco: any) => {
      clearDisposables();
      editor.updateOptions({
        glyphMargin: true,
      });
      syncBreakpointsToModel();

      editorDisposablesRef.current.push(
        editor.onMouseDown((event: any) => {
          const isGutter = event.target?.type === monaco.editor.MouseTargetType.GUTTER_GLYPH_MARGIN;
          const lineNumber = event.target?.position?.lineNumber;
          if (!isGutter || typeof lineNumber !== "number") {
            return;
          }
          event.event.preventDefault();
          toggleDebugBreakpoint(lineNumber);
        }),
      );
      editorDisposablesRef.current.push(
        editor.onKeyDown((event: any) => {
          if (event.keyCode === monaco.KeyCode.F9) {
            event.preventDefault();
            const lineNumber = editor.getPosition()?.lineNumber;
            if (typeof lineNumber === "number") {
              toggleDebugBreakpoint(lineNumber);
            }
          }
        }),
      );

      applyBreakpointDecorations();
    },
    [applyBreakpointDecorations, clearDisposables, syncBreakpointsToModel, toggleDebugBreakpoint],
  );

  return {
    debugBreakpoints,
    clearDebugBreakpoints,
    onEditorReadyForDebug,
  };
}
