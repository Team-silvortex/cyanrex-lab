import { useEffect, useState } from "react";

import type { CDiagnostic } from "../../utils/cAnalyzer";
import type { EbpfCheckResponse } from "./models";
import { MAX_UPLOAD_BYTES } from "./models";

export type CompilerStatus = "idle" | "checking" | "passed" | "issues" | "unavailable";

export function useCompilerDiagnostics(code: string, engineUrl: string) {
  const [diagnostics, setDiagnostics] = useState<CDiagnostic[]>([]);
  const [status, setStatus] = useState<CompilerStatus>("idle");

  useEffect(() => {
    if (!code.trim() || code.length > MAX_UPLOAD_BYTES) {
      setDiagnostics([]);
      setStatus("idle");
      return;
    }

    const controller = new AbortController();
    const timer = window.setTimeout(async () => {
      setStatus("checking");
      try {
        const response = await fetch(`${engineUrl}/ebpf/check`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          signal: controller.signal,
          body: JSON.stringify({ code }),
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const result = (await response.json()) as EbpfCheckResponse;
        setDiagnostics(result.diagnostics.map((item) => ({
          line: item.line,
          column: item.column,
          endColumn: item.end_column,
          severity: item.severity === "note" ? "info" : item.severity,
          message: `clang: ${item.message}`,
        })));
        setStatus(result.ok ? "passed" : "issues");
      } catch (error) {
        if ((error as Error).name !== "AbortError") {
          setDiagnostics([]);
          setStatus("unavailable");
        }
      }
    }, 700);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [code, engineUrl]);

  return { diagnostics, status };
}
