import { analyzeCCode } from "../../utils/cAnalyzer";

export function applyMarkers(editor: any, monaco: any, diagnostics: ReturnType<typeof analyzeCCode>["diagnostics"]) {
  const model = editor.getModel?.();
  if (!model) return;

  const markers = diagnostics.map((d) => ({
    startLineNumber: d.line,
    startColumn: d.column,
    endLineNumber: d.line,
    endColumn: d.endColumn,
    message: d.message,
    severity:
      d.severity === "error"
        ? monaco.MarkerSeverity.Error
        : d.severity === "warning"
          ? monaco.MarkerSeverity.Warning
          : monaco.MarkerSeverity.Info,
  }));

  monaco.editor.setModelMarkers(model, "cyanrex-c-analyzer", markers);
}

export function toIncludePath(includeHint: string): string {
  const match = includeHint.match(/[<"]([^>"]+)[>"]/);
  return match ? match[1] : includeHint;
}
