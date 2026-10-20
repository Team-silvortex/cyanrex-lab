import { useCallback, useEffect, useRef, useState } from "react";
import { readHelperSnapshot, type HelperSnapshot } from "./helperRequest";

type State = { scope: string; snapshot: HelperSnapshot | null; loading: boolean; errorKey: string };
type Owner = { scope: string; controller: AbortController; check: () => Promise<void> };
const initial = (scope: string): State => ({ scope, snapshot: null, loading: false, errorKey: "" });

/** Page-owned, explicitly requested diagnostics. No persistence, polling or deployment authority. */
export function useEnvironmentCheck(engineUrl: string, navigation: string) {
  const scope = JSON.stringify([engineUrl, navigation]), currentScope = useRef(scope);
  currentScope.current = scope;
  const owner = useRef<Owner | null>(null);
  const [state, setState] = useState(() => initial(scope));
  const view = state.scope === scope ? state : initial(scope);

  useEffect(() => {
    const controller = new AbortController();
    let inFlight = false;
    const owns = () => owner.current?.controller === controller && currentScope.current === scope && !controller.signal.aborted;
    const check = async () => {
      if (!owns() || inFlight) return;
      inFlight = true;
      setState({ scope, snapshot: null, loading: true, errorKey: "" });
      try {
        const snapshot = await readHelperSnapshot(engineUrl, controller.signal);
        if (owns()) setState({ scope, snapshot, loading: false, errorKey: "" });
      } catch {
        if (owns()) setState({ scope, snapshot: null, loading: false, errorKey: "helper.checkFailed" });
      } finally { inFlight = false; }
    };
    owner.current = { scope, controller, check };
    setState(initial(scope));
    return () => {
      controller.abort();
      if (owner.current?.controller === controller) owner.current = null;
    };
  }, [engineUrl, scope]);

  const runCheck = useCallback(() => {
    const current = owner.current;
    return current?.scope === scope && currentScope.current === scope ? current.check() : Promise.resolve();
  }, [scope]);
  return { ...view, runCheck };
}
