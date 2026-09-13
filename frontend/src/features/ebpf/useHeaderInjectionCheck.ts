import { useEffect, useMemo, useRef, useState } from "react";
import { MAX_UPLOAD_BYTES } from "./models";
import { runCompilerCheck } from "./compilerCheck";

type CheckState = {
  status: "idle" | "checking" | "passed" | "issues" | "error";
  message: string; stdout: string; stderr: string; diagnostics: number;
};
const IDLE: CheckState = { status: "idle", message: "", stdout: "", stderr: "", diagnostics: 0 };

export function useHeaderInjectionCheck(
  code: string, engineUrl: string, headerContextKey: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  const key = useMemo(() => JSON.stringify([engineUrl, headerContextKey, code]), [engineUrl, headerContextKey, code]);
  const [view, setView] = useState({ key, value: IDLE });
  const request = useRef<AbortController | null>(null);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    setView({ key, value: IDLE });
    return () => { mounted.current = false; request.current?.abort(); request.current = null; };
  }, [key]);

  const run = async () => {
    if (!mounted.current || request.current) return;
    const show = (value: CheckState) => setView({ key, value });
    if (!code.trim()) { show({ ...IDLE, status: "error", message: t("ebpf.headerInjectionCheckEmpty") }); return; }
    if (new TextEncoder().encode(code).byteLength > MAX_UPLOAD_BYTES) {
      show({ ...IDLE, status: "error", message: t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }) }); return;
    }
    const controller = new AbortController(); request.current = controller;
    show({ ...IDLE, status: "checking", message: t("ebpf.headerInjectionCheckRunning") });
    try {
      // Header injection is always checked by this Engine, never the optional Agent.
      const result = await runCompilerCheck(code, engineUrl, "local", controller.signal);
      if (controller.signal.aborted || request.current !== controller) return;
      show({ status: result.ok ? "passed" : "issues",
        message: result.message?.trim() || t(result.ok ? "ebpf.headerInjectionCheckPassed" : "ebpf.headerInjectionCheckFailed"),
        stdout: result.stdout || "", stderr: result.stderr || "", diagnostics: result.diagnostics.length });
    } catch (error) {
      if (!controller.signal.aborted && request.current === controller) {
        show({ ...IDLE, status: "error", message: (error as Error).message });
      }
    } finally { if (request.current === controller) request.current = null; }
  };
  return { headerInjectionCheck: view.key === key ? view.value : IDLE, runHeaderInjectionSelfCheck: run };
}
