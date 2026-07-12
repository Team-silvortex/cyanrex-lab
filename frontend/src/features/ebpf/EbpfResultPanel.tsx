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
        <p><strong>success:</strong> {String(result.success)}</p>
        <p><strong>stage:</strong> {sanitizeForDisplay(result.stage)}</p>
        <p><strong>message:</strong> {sanitizeForDisplay(result.message)}</p>
        <p><strong>pin_path:</strong> {sanitizeForDisplay(result.pin_path || "(none)")}</p>
        <h4>{t("ebpf.compileStdout")}</h4>
        <pre>{sanitizeForDisplay(result.compile_stdout || "(empty)")}</pre>
        <h4>{t("ebpf.compileStderr")}</h4>
        <pre>{sanitizeForDisplay(result.compile_stderr || "(empty)")}</pre>
        <h4>{t("ebpf.loadStdout")}</h4>
        <pre>{sanitizeForDisplay(result.load_stdout || "(empty)")}</pre>
        <h4>{t("ebpf.loadStderr")}</h4>
        <pre>{sanitizeForDisplay(result.load_stderr || "(empty)")}</pre>
      </>}
    </section>
  );
}
