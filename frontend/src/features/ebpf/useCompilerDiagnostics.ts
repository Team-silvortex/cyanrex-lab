import { useEffect, useState } from "react";

import type { CDiagnostic } from "../../utils/cAnalyzer";
import type {
  EbpfCheckResponse,
  EbpfCompilerTarget,
  EbpfRemoteCheckResponse,
} from "./models";
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

export function useCompilerDiagnostics(
  code: string,
  engineUrl: string,
  headerContextKey = "",
  target: EbpfCompilerTarget = "local",
) {
  const [diagnostics, setDiagnostics] = useState<CDiagnostic[]>([]);
  const [status, setStatus] = useState<CompilerStatus>("idle");

  useEffect(() => {
    if (!code.trim() || code.length > MAX_UPLOAD_BYTES) {
      setDiagnostics([]);
      setStatus("idle");
      return;
    }

    const cacheKey = hashCode(`${target}//${code}//${headerContextKey}`);
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
        const request = runCompilerCheck(cacheKey, code, engineUrl, target, controller.signal);
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
  }, [code, engineUrl, headerContextKey, target]);

  return { diagnostics, status };
}

async function runCompilerCheck(
  cacheKey: string,
  code: string,
  engineUrl: string,
  target: EbpfCompilerTarget,
  signal: AbortSignal,
): Promise<EbpfCheckResponse> {
  const inFlight = inFlightChecks.get(cacheKey);
  if (inFlight) return inFlight;

  if (target.startsWith("agent:")) {
    return runRemoteCompilerCheck(code, engineUrl, target.slice(6), signal);
  }

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

async function runRemoteCompilerCheck(
  code: string,
  engineUrl: string,
  agentId: string,
  signal: AbortSignal,
): Promise<EbpfCheckResponse> {
  let jobId = "";
  let completed = false;
  try {
    const submitted = await fetchJson<EbpfRemoteCheckResponse>(`${engineUrl}/ebpf/check/remote`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      signal,
      body: JSON.stringify({ code, agent_id: agentId, program_name: "inline-check" }),
    });
    jobId = submitted.job_id;
    const deadline = Date.now() + 35_000;
    while (Date.now() < deadline) {
      const status = await fetchJson<EbpfRemoteCheckResponse>(
        `${engineUrl}/ebpf/check/remote?job_id=${encodeURIComponent(jobId)}`,
        { credentials: "include", signal },
      );
      if (status.result) {
        completed = true;
        return status.result;
      }
      if (["succeeded", "failed", "cancelled", "expired"].includes(status.state)) {
        throw new Error(status.message || `remote check ended as ${status.state}`);
      }
      await abortableDelay(500, signal);
    }
    throw new Error("remote compiler check exceeded 35 seconds");
  } finally {
    if (jobId && !completed) {
      void cancelRemoteCheck(engineUrl, jobId);
    }
  }
}

async function fetchJson<T>(url: string, init: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  if (!response.ok) {
    const payload = await response.json().catch(() => ({})) as { message?: string };
    throw new Error(payload.message || `HTTP ${response.status}`);
  }
  return (await response.json()) as T;
}

async function cancelRemoteCheck(engineUrl: string, jobId: string) {
  try {
    await fetch(`${engineUrl}/ebpf/check/remote/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      keepalive: true,
      body: JSON.stringify({ job_id: jobId }),
    });
  } catch {
    // The server-side lease deadline remains the final cleanup boundary.
  }
}

function abortableDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    const onAbort = () => {
      window.clearTimeout(timer);
      reject(new DOMException("Aborted", "AbortError"));
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function hashCode(value: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `${hash.toString(16)}-${value.length}`;
}
