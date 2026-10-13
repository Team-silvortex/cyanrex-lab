import { analyzeCCode } from "../../utils/cAnalyzer";
import { localDiagnosticCode, localizeLegacyText, type Translate } from "../../i18n/legacyText";

export function applyMarkers(editor: any, monaco: any, diagnostics: ReturnType<typeof analyzeCCode>["diagnostics"], translate?: Translate) {
  const model = editor.getModel?.();
  if (!model) return;

  const markers = diagnostics.map((d) => ({
    startLineNumber: d.line,
    startColumn: d.column,
    endLineNumber: d.line,
    endColumn: d.endColumn,
    message: translate ? localizeLegacyText(d.message, translate) : d.message,
    code: localDiagnosticCode(d.message),
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
