import { useEffect, useMemo, useRef, useState } from "react";

import type { CDiagnostic } from "../../utils/cAnalyzer";
import type { EbpfCompilerTarget } from "./models";
import { MAX_UPLOAD_BYTES } from "./models";
import { runCompilerCheck } from "./compilerCheck";

export type CompilerStatus = "idle" | "checking" | "passed" | "issues" | "unavailable";

type CachedDiagnostics = {
  status: CompilerStatus;
  diagnostics: CDiagnostic[];
  expiresAt: number;
};

const DIAGNOSTIC_CACHE_TTL_MS = 8_000;
const DIAGNOSTIC_CACHE_MAX_ENTRIES = 24;
const IDLE = { diagnostics: [] as CDiagnostic[], status: "idle" as CompilerStatus };

export function useCompilerDiagnostics(
  code: string,
  engineUrl: string,
  headerContextKey = "",
  target: EbpfCompilerTarget = "local",
) {
  // No global reuse across editors, navigation or logout/login remounts. Server
  // caches remain owner-scoped; this cache is only a short-lived editing aid.
  const cache = useRef(new Map<string, CachedDiagnostics>());
  const cacheKey = useMemo(
    () => JSON.stringify([engineUrl, target, headerContextKey, code]),
    [engineUrl, target, headerContextKey, code],
  );
  const [view, setView] = useState({ key: cacheKey, ...IDLE });

  useEffect(() => {
    if (!code.trim() || code.length > MAX_UPLOAD_BYTES) {
      setView({ key: cacheKey, ...IDLE });
      return;
    }

    const cached = cache.current.get(cacheKey);
    if (cached && cached.expiresAt > Date.now()) {
      setView({ key: cacheKey, diagnostics: cached.diagnostics, status: cached.status });
      return;
    }

    const controller = new AbortController();
    setView({ key: cacheKey, ...IDLE });
    const delay = code.length > 20_000 ? 1200 : 700;
    const timer = window.setTimeout(async () => {
      setView({ key: cacheKey, diagnostics: [], status: "checking" });
      try {
        const result = await runCompilerCheck(code, engineUrl, target, controller.signal);
        // Aborting transport cannot retract an already-fulfilled continuation.
        if (controller.signal.aborted) return;
        const mapped: CDiagnostic[] = result.diagnostics.map((item): CDiagnostic => ({
          line: item.line,
          column: item.column,
          endColumn: item.end_column,
          severity: item.severity === "note" ? "info" : item.severity,
          message: `clang: ${item.message}`,
        }));
        const nextStatus = result.ok ? "passed" : "issues";
        setView({ key: cacheKey, diagnostics: mapped, status: nextStatus });
        cache.current.delete(cacheKey);
        cache.current.set(cacheKey, {
          status: nextStatus,
          diagnostics: mapped,
          expiresAt: Date.now() + DIAGNOSTIC_CACHE_TTL_MS,
        });
        if (cache.current.size > DIAGNOSTIC_CACHE_MAX_ENTRIES) {
          const oldest = cache.current.keys().next().value;
          if (oldest) cache.current.delete(oldest);
        }
      } catch {
        if (!controller.signal.aborted) {
          setView({ key: cacheKey, diagnostics: [], status: "unavailable" });
        }
      }
    }, delay);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [cacheKey, code, engineUrl, target]);

  // Hide stale markers even in the render before the previous effect cleans up.
  return view.key === cacheKey ? { diagnostics: view.diagnostics, status: view.status } : IDLE;
}
