import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/context";
import { requestSettings } from "../settings/settingsRequest";
import type { RunnerAdminNotice, RunnerAgentInventory, RunnerAgentView, RunnerJobInventory, RunnerJobView } from "./models";
import { cancelIdentity, parseAgentInventory, parseCancelAcknowledgement, parseJobInventory, parseProbeAcknowledgement, probeIdentity } from "./runnerInventory";

type State = {
  scope: string; agents: RunnerAgentInventory | null; jobs: RunnerJobInventory | null;
  loading: boolean; refreshing: boolean; actionId: string; stale: boolean; verify: boolean;
  errorKey: string; notice: RunnerAdminNotice | null;
};
type Action = { kind: "probe"; target: RunnerAgentView } | { kind: "cancel"; target: RunnerJobView };
type Generation = { scope: string; controller: AbortController; refresh: (explicit: boolean) => Promise<void>;
  act: (action: Action, signal: AbortSignal) => Promise<void> };
const initial = (scope: string): State => ({ scope, agents: null, jobs: null, loading: true, refreshing: true,
  actionId: "", stale: false, verify: false, errorKey: "", notice: null });
const ready = (state: State) => Boolean(state.agents?.enabled && state.jobs && !state.loading
  && !state.refreshing && !state.actionId && !state.stale && !state.verify);

export function useRunnerAgentAdmin(engineUrl: string, navigation = "") {
  const { t } = useI18n();
  const translate = useRef(t); translate.current = t;
  const scope = JSON.stringify([engineUrl, navigation]);
  const currentScope = useRef(scope); currentScope.current = scope;
  const generation = useRef<Generation | null>(null);
  const [state, setState] = useState(() => initial(scope));
  const view = state.scope === scope ? state : initial(scope);

  useEffect(() => {
    const controller = new AbortController();
    let snapshot = initial(scope), sampledAt = 0;
    let read: { controller: AbortController; promise: Promise<void> } | null = null;
    let actionController: AbortController | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const owns = () => generation.current?.controller === controller && currentScope.current === scope && !controller.signal.aborted;
    const publish = (patch: Partial<State>) => { if (owns()) { snapshot = { ...snapshot, ...patch }; setState(snapshot); } };
    const schedule = () => { clearTimeout(timer); timer = setTimeout(() => { void refresh(false); }, 10_000); };

    const refresh = (explicit: boolean): Promise<void> => {
      if (!owns() || snapshot.actionId) return Promise.resolve();
      if (read) return read.promise;
      clearTimeout(timer);
      const request = { controller: new AbortController(), promise: Promise.resolve() }, started = performance.now();
      read = request;
      publish({ refreshing: true });
      const ownsRead = () => owns() && read === request;
      request.promise = (async () => {
        try {
          const [agents, jobs] = await Promise.all([
            requestSettings(`${engineUrl}/runner/agents`, request.controller.signal, parseAgentInventory),
            requestSettings(`${engineUrl}/runner/jobs`, request.controller.signal, parseJobInventory),
          ]);
          if (ownsRead()) {
            sampledAt = started;
            publish({ agents, jobs, stale: false, errorKey: "", verify: explicit ? false : snapshot.verify });
          }
        } catch {
          if (ownsRead()) publish({ stale: Boolean(snapshot.agents), errorKey: "settings.runnerReadFailed" });
        } finally {
          // A failed half of the atomic pair also cancels its pending sibling.
          request.controller.abort();
          if (ownsRead()) { read = null; publish({ loading: false, refreshing: false }); schedule(); }
        }
      })();
      return request.promise;
    };

    const act = async (action: Action, signal: AbortSignal) => {
      signal.throwIfAborted();
      const failReview = () => { throw new Error(translate.current("settings.runnerReviewChanged")); };
      if (!owns() || !ready(snapshot) || performance.now() - sampledAt >= 20_000) return failReview();
      if (action.kind === "probe") {
        const target = snapshot.agents!.agents.find(agent => agent.agent_id === action.target.agent_id);
        // Use Engine-relative expiry plus elapsed browser time, not synchronized wall clocks.
        const estimatedServerTime = Date.parse(snapshot.agents!.generated_at) + performance.now() - sampledAt;
        if (!target || target.state !== "healthy" || probeIdentity(target) !== probeIdentity(action.target)
          || Date.parse(target.expires_at) <= estimatedServerTime) return failReview();
      } else {
        const target = snapshot.jobs!.jobs.find(job => job.job_id === action.target.job_id);
        if (!target || !["queued", "claimed"].includes(target.state) || target.state !== action.target.state
          || target.assigned_agent_id !== action.target.assigned_agent_id
          || cancelIdentity(target) !== cancelIdentity(action.target)) return failReview();
      }
      clearTimeout(timer);
      const ownedAction = new AbortController(), abort = () => ownedAction.abort(signal.reason);
      actionController = ownedAction;
      signal.addEventListener("abort", abort, { once: true });
      const id = action.kind === "probe" ? action.target.agent_id : action.target.job_id;
      publish({ actionId: `${action.kind}:${id}`, errorKey: "", notice: null, stale: true });
      let acknowledged = false;
      try {
        if (action.kind === "probe") {
          await requestSettings(`${engineUrl}/runner/jobs/probe`, ownedAction.signal,
            value => parseProbeAcknowledgement(value, action.target.agent_id),
            { agent_id: action.target.agent_id, message: "teacher health probe", timeout_seconds: 30 });
        } else {
          await requestSettings(`${engineUrl}/runner/jobs/cancel`, ownedAction.signal,
            value => parseCancelAcknowledgement(value, action.target), { job_id: action.target.job_id });
        }
        if (!owns()) return;
        acknowledged = true;
        publish({ notice: { kind: action.kind === "probe" ? "probe_submitted" : "cancel_requested", subject: id } });
      } catch {
        if (owns()) publish({ verify: true });
        throw new Error(translate.current("settings.runnerActionUnconfirmed"));
      } finally {
        signal.removeEventListener("abort", abort);
        if (owns()) {
          actionController = null;
          publish({ actionId: "" });
          // Acknowledgement is independent of read-back success. Never replay a POST.
          if (acknowledged) void refresh(false); else schedule();
        }
      }
    };

    generation.current = { scope, controller, refresh, act };
    setState(snapshot);
    void refresh(false);
    return () => {
      controller.abort(); read?.controller.abort(); actionController?.abort(); clearTimeout(timer);
      if (generation.current?.controller === controller) generation.current = null;
    };
  }, [engineUrl, scope]);

  const refresh = useCallback(() => {
    const owner = generation.current;
    return owner?.scope === scope && currentScope.current === scope ? owner.refresh(true) : Promise.resolve();
  }, [scope]);
  const act = useCallback((action: Action, signal: AbortSignal) => {
    const owner = generation.current;
    if (owner?.scope !== scope || currentScope.current !== scope) return Promise.reject(new Error(translate.current("settings.runnerReviewChanged")));
    return owner.act(action, signal);
  }, [scope]);
  return { ...view, canAct: ready(view), error: view.verify ? t("settings.runnerActionUnconfirmed") : view.errorKey ? t(view.errorKey) : "", refresh,
    probeAgent: (target: RunnerAgentView, signal: AbortSignal) => act({ kind: "probe", target }, signal),
    cancelJob: (target: RunnerJobView, signal: AbortSignal) => act({ kind: "cancel", target }, signal) };
}
