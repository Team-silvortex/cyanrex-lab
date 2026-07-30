import { useEffect, useState } from "react";

import type { CDiagnostic } from "../../utils/cAnalyzer";
import type { EbpfCheckResponse } from "./models";
import { MAX_UPLOAD_BYTES } from "./models";

export type CompilerStatus = "idle" | "checking" | "passed" | "issues" | "unavailable";

type CachedDiagnostics = {
  status: CompilerStatus;
  diagnostics: CDiagnostic[];
  expiresAt: number;
};

const DIAGNOSTIC_CACHE_TTL_MS = 8_000;
const DIAGNOSTIC_CACHE_MAX_ENTRIES = 24;
const inFlightChecks = new Map<string, Promise<EbpfCheckResponse>>();
const diagnosticCache = new Map<string, CachedDiagnostics>();

export function useCompilerDiagnostics(code: string, engineUrl: string) {
  const [diagnostics, setDiagnostics] = useState<CDiagnostic[]>([]);
  const [status, setStatus] = useState<CompilerStatus>("idle");

  useEffect(() => {
    if (!code.trim() || code.length > MAX_UPLOAD_BYTES) {
      setDiagnostics([]);
      setStatus("idle");
      return;
    }

    const cacheKey = hashCode(code);
    const now = Date.now();
    const cached = diagnosticCache.get(cacheKey);
    if (cached && cached.expiresAt > now) {
      setDiagnostics(cached.diagnostics);
      setStatus(cached.status);
      return;
    }

    const controller = new AbortController();
    const delay = code.length > 20_000 ? 1200 : 700;
    const timer = window.setTimeout(async () => {
      setStatus("checking");
      try {
        const request = runCompilerCheck(cacheKey, code, engineUrl, controller.signal);
        inFlightChecks.set(cacheKey, request);
        const response = await request;
        const result = response;
        const mapped: CDiagnostic[] = result.diagnostics.map((item): CDiagnostic => ({
          line: item.line,
          column: item.column,
          endColumn: item.end_column,
          severity: item.severity === "note" ? "info" : item.severity,
          message: `clang: ${item.message}`,
        }));
        const nextStatus = result.ok ? "passed" : "issues";
        setDiagnostics(mapped);
        setStatus(nextStatus);
        diagnosticCache.set(cacheKey, {
          status: nextStatus,
          diagnostics: mapped,
          expiresAt: Date.now() + DIAGNOSTIC_CACHE_TTL_MS,
        });
        if (diagnosticCache.size > DIAGNOSTIC_CACHE_MAX_ENTRIES) {
          const oldest = diagnosticCache.keys().next().value;
          if (oldest) diagnosticCache.delete(oldest);
        }
      } catch (error) {
        if ((error as Error).name !== "AbortError") {
          setDiagnostics([]);
          setStatus("unavailable");
        }
      } finally {
        inFlightChecks.delete(cacheKey);
      }
    }, delay);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
      inFlightChecks.delete(cacheKey);
    };
  }, [code, engineUrl]);

  return { diagnostics, status };
}

async function runCompilerCheck(
  cacheKey: string,
  code: string,
  engineUrl: string,
  signal: AbortSignal,
): Promise<EbpfCheckResponse> {
  const inFlight = inFlightChecks.get(cacheKey);
  if (inFlight) return inFlight;

  return (async () => {
    const response = await fetch(`${engineUrl}/ebpf/check`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      signal,
      body: JSON.stringify({ code }),
    });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return (await response.json()) as EbpfCheckResponse;
  })();
}

function hashCode(value: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `${hash.toString(16)}-${value.length}`;
}
