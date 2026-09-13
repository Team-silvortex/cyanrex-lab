import { useEffect, useRef, useState } from "react";
import { buildDetachBody } from "./detachTarget";
import type { EbpfRunResponse } from "./models";
import { parseDetachResult, parseRunResult, requestRuntimeJson, RuntimeHttpError } from "./runtimeRequest";

type Translate = (key: string, vars?: Record<string, string | number>) => string;
type Operation = { kind: "run" | "detach"; controller: AbortController; epoch: number; scope: string; release: () => void };
type View = { epoch: number; result: EbpfRunResponse | null; error: string | null; notice: string; started: boolean };

export function useRuntimeActions(engineUrl: string, navigationKey: string, contextKey: string,
  invalidateAttachments: () => void, refreshAttachments: () => Promise<boolean>, t: Translate) {
  const scope = JSON.stringify([engineUrl, navigationKey]);
  const context = useRef({ key: contextKey, epoch: 0 });
  if (context.current.key !== contextKey) context.current = { key: contextKey, epoch: context.current.epoch + 1 };
  const epoch = context.current.epoch;
  const currentScope = useRef(scope); currentScope.current = scope;
  const mounted = useRef(false), active = useRef<Operation | null>(null);
  const [busy, setBusy] = useState<Operation | null>(null);
  const empty = (): View => ({ epoch, result: null, error: null, notice: "", started: false });
  const [view, setView] = useState<View>(empty);
  useEffect(() => {
    mounted.current = true; setBusy(null);
    return () => {
      mounted.current = false; active.current?.controller.abort(); active.current?.release(); active.current = null;
    };
  }, [scope]);
  const isBusy = () => active.current !== null;
  const current = (operation: Operation) => mounted.current && active.current === operation
    && !operation.controller.signal.aborted && currentScope.current === operation.scope;
  const publishable = (operation: Operation) => current(operation) && context.current.epoch === operation.epoch;
  const begin = (kind: Operation["kind"], parent?: AbortSignal) => {
    if (!mounted.current || isBusy() || parent?.aborted || currentScope.current !== scope || context.current.epoch !== epoch) {
      throw new Error(t("ebpf.runtimeActionBlocked"));
    }
    const controller = new AbortController(), cancel = () => controller.abort(parent?.reason);
    parent?.addEventListener("abort", cancel, { once: true });
    const operation = { kind, controller, epoch, scope, release: () => parent?.removeEventListener("abort", cancel) };
    active.current = operation; setBusy(operation); invalidateAttachments();
    setView(previous => ({ ...(kind === "run" || previous.epoch !== epoch ? empty() : previous), error: null, notice: "", started: true }));
    return operation;
  };
  const finish = (operation: Operation) => {
    operation.release();
    if (active.current !== operation) return;
    const refresh = current(operation);
    active.current = null;
    if (mounted.current) setBusy(null);
    // Read-only reconciliation is independent of the mutation's completion/UI lock.
    if (refresh) void refreshAttachments();
  };
  const failure = (operation: Operation, error: unknown) => {
    const rejected = error instanceof RuntimeHttpError && [400, 401, 403, 404, 413, 429].includes(error.status);
    const detail = error instanceof Error ? error.message : String(error);
    const message = rejected ? detail : `${t("ebpf.runtimeUnconfirmed")} ${detail}`;
    if (publishable(operation)) setView(previous => ({ ...previous, error: message }));
    return new Error(message);
  };
  const run = async (payload: unknown, parent?: AbortSignal, explain = (message: string) => message) => {
    const operation = begin("run", parent);
    try {
      const result = parseRunResult(await requestRuntimeJson(`${engineUrl}/ebpf/run`, operation.controller.signal, payload));
      if (!publishable(operation)) return false;
      setView({ epoch, result, error: result.success ? null : explain(result.message), notice: "", started: true });
      return true;
    } catch (error) {
      // Engine source validation is a normal failed run report, not an unknown mutation.
      if (error instanceof RuntimeHttpError && error.status === 400) {
        let rejected: EbpfRunResponse | null = null;
        try { rejected = parseRunResult(error.payload); } catch { /* Non-result HTTP errors keep their failure path. */ }
        if (rejected && !rejected.success && publishable(operation)) {
          setView({ epoch, result: rejected, error: explain(rejected.message), notice: "", started: true });
          return true;
        }
      }
      throw failure(operation, error);
    }
    finally { finish(operation); }
  };
  const detach = async (pinPath: string | null, parent?: AbortSignal) => {
    const body = buildDetachBody(pinPath);
    const operation = begin("detach", parent);
    try {
      const result = parseDetachResult(await requestRuntimeJson(`${engineUrl}/ebpf/detach`, operation.controller.signal, body));
      if (!publishable(operation)) return;
      if (!result.ok || result.clean !== true) throw new Error(
        `${t("ebpf.detachUnconfirmed")} ${result.message} ${(result.safety_notes ?? []).join(" | ")}`,
      );
      const notice = `${t("ebpf.detachedCount", { count: result.detached.length })} | ${t("ebpf.detachState", { state: t("ebpf.detachClean") })}`;
      setView(previous => {
        const old = previous.epoch === epoch ? previous.result : null;
        const retired = old?.pin_path && (pinPath === null || result.detached.includes(old.pin_path));
        return { epoch, error: null, notice, started: true, result: old ? { ...old,
          ...(retired ? { pin_path: null, debug: null } : {}), message: `${old.message} | ${notice}` } : null };
      });
    } catch (error) { throw failure(operation, error); }
    finally { finish(operation); }
  };
  return {
    result: view.epoch === epoch ? view.result : null, error: view.epoch === epoch ? view.error : null,
    runtimeNotice: view.epoch !== epoch && view.started ? t("ebpf.runtimeContextChanged") : view.notice,
    running: busy?.scope === scope && busy.kind === "run", detaching: busy?.scope === scope && busy.kind === "detach",
    isBusy, run, detach, clear: () => setView(empty()),
  };
}
