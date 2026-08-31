import { sanitizeForDisplay } from "../../utils/security";
import type { EbpfRunResponse } from "./models";

type Props = {
  result: EbpfRunResponse | null;
  error: string | null;
  t: (key: string) => string;
};

export default function EbpfResultPanel({ result, error, t }: Props) {
  return (
    <section className="panel" style={{ marginTop: 16 }}>
      <h3>{t("ebpf.result")}</h3>
      {!result && !error && <p className="meta">{t("ebpf.noRunResult")}</p>}
      {error && <p className="error">{sanitizeForDisplay(error)}</p>}
      {result && <>
        <p><strong>{t("ebpf.resultSuccess")}</strong> {String(result.success)}</p>
        <p><strong>{t("ebpf.resultStage")}</strong> {sanitizeForDisplay(result.stage)}</p>
        <p><strong>{t("ebpf.resultMessage")}</strong> {sanitizeForDisplay(result.message)}</p>
        <p><strong>{t("ebpf.resultPinPath")}</strong> {sanitizeForDisplay(result.pin_path || t("ebpf.noData"))}</p>
        {result.debug && <div className="panel" style={{ margin: "12px 0", background: "#0b1425" }}>
          <p><strong>{t("ebpf.debugMode")}</strong> {sanitizeForDisplay(result.debug.mode)}</p>
          <p>
            <strong>{t("ebpf.debugInstrumentedLines")}</strong>{" "}
            {result.debug.instrumented_lines.length > 0
              ? result.debug.instrumented_lines.map((line) => `L${line}`).join(", ")
              : t("ebpf.debugNoInstrumentedLines")}
          </p>
          {result.debug.rejected.length > 0 && <>
            <p><strong>{t("ebpf.debugRejectedLines")}</strong></p>
            <ul>
              {result.debug.rejected.map((item) => (
                <li key={`${item.line}:${item.reason}`}>
                  L{item.line}: {sanitizeForDisplay(item.reason)}
                </li>
              ))}
            </ul>
          </>}
        </div>}
        <h4>{t("ebpf.compileStdout")}</h4>
        <pre>{sanitizeForDisplay(result.compile_stdout || t("ebpf.outputEmpty"))}</pre>
        <h4>{t("ebpf.compileStderr")}</h4>
        <pre>{sanitizeForDisplay(result.compile_stderr || t("ebpf.outputEmpty"))}</pre>
        <h4>{t("ebpf.loadStdout")}</h4>
        <pre>{sanitizeForDisplay(result.load_stdout || t("ebpf.outputEmpty"))}</pre>
        <h4>{t("ebpf.loadStderr")}</h4>
        <pre>{sanitizeForDisplay(result.load_stderr || t("ebpf.outputEmpty"))}</pre>
      </>}
    </section>
  );
}
