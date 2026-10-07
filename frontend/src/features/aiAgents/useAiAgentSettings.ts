import { useEffect, useRef, useState } from "react";
import { AiAgentSettingsConflict, requestAiAgentSettings, reviewAiAgentSettings,
  type AiAgentSettings, type AiAgentSettingsUpdate } from "./aiAgentSettings";

type State = {
  scope: string; settings: AiAgentSettings | null; draft: AiAgentSettings | null;
  loading: boolean; saving: boolean; stale: boolean; errorKey: string; messageKey: string;
};
type Review = { scope: string; generation: AbortController; body: AiAgentSettingsUpdate };
const initial = (scope: string): State => ({ scope, settings: null, draft: null,
  loading: true, saving: false, stale: true, errorKey: "", messageKey: "" });

export function useAiAgentSettings(engineUrl: string, navigation: string, t: (key: string) => string) {
  const scope = JSON.stringify([engineUrl, navigation]), currentScope = useRef(scope); currentScope.current = scope;
  const generation = useRef<AbortController | null>(null), mutation = useRef<AbortController | null>(null);
  const blocked = useRef(true), [reloadId, setReloadId] = useState(0);
  const [state, setState] = useState(() => initial(scope)), current = useRef(state);
  const update = (next: State) => { current.current = next; setState(next); };
  const view = state.scope === scope ? state : initial(scope);

  useEffect(() => {
    const controller = new AbortController(); generation.current = controller; blocked.current = true;
    update({ ...(current.current.scope === scope ? current.current : initial(scope)),
      loading: true, saving: false, stale: true, errorKey: "", messageKey: "" });
    const owns = () => generation.current === controller && currentScope.current === scope && !controller.signal.aborted;
    void (async () => {
      try {
        const settings = await requestAiAgentSettings(`${engineUrl}/settings/ai-agents`, controller.signal);
        if (!owns()) return;
        blocked.current = false;
        update({ ...current.current, settings, draft: structuredClone(settings), loading: false, stale: false });
      } catch {
        if (owns()) update({ ...current.current, loading: false, stale: true, errorKey: "aiAgents.loadFailed" });
      }
    })();
    return () => {
      controller.abort();
      if (generation.current === controller) {
        generation.current = null; blocked.current = true; mutation.current?.abort(); mutation.current = null;
      }
    };
  }, [engineUrl, scope, reloadId]);

  const ready = state.scope === scope && !!view.draft && !view.loading && !view.saving && !view.stale;
  const edit = (change: (draft: AiAgentSettings) => AiAgentSettings) => {
    if (!ready || blocked.current || currentScope.current !== scope || !current.current.draft) return;
    update({ ...current.current, draft: change(current.current.draft), errorKey: "", messageKey: "" });
  };
  const review = (): Review | null => {
    if (!ready || blocked.current || currentScope.current !== scope || !generation.current) return null;
    try { return { scope, generation: generation.current, body: reviewAiAgentSettings(current.current.draft) }; }
    catch { update({ ...current.current, errorKey: "aiAgents.invalidDraft", messageKey: "" }); return null; }
  };
  const save = async (snapshot: Review, parent: AbortSignal) => {
    parent.throwIfAborted();
    if (blocked.current || mutation.current || snapshot.scope !== currentScope.current
      || snapshot.generation !== generation.current || snapshot.generation.signal.aborted
      || JSON.stringify(snapshot.body) !== JSON.stringify(reviewAiAgentSettings(current.current.draft))) {
      throw new Error(t("aiAgents.reviewChanged"));
    }
    const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
    parent.addEventListener("abort", cancel, { once: true }); mutation.current = controller; blocked.current = true;
    const owns = () => generation.current === snapshot.generation && currentScope.current === snapshot.scope && mutation.current === controller;
    update({ ...current.current, saving: true, errorKey: "", messageKey: "" });
    try {
      const settings = await requestAiAgentSettings(`${engineUrl}/settings/ai-agents`, controller.signal, snapshot.body);
      controller.signal.throwIfAborted();
      if (owns()) {
        blocked.current = false;
        update({ ...current.current, settings, draft: structuredClone(settings), stale: false, messageKey: "aiAgents.saved" });
      }
    } catch (error) {
      const key = error instanceof AiAgentSettingsConflict ? "aiAgents.conflict" : "aiAgents.saveUnconfirmed";
      if (owns()) update({ ...current.current, stale: true, errorKey: key, messageKey: "" });
      throw new Error(t(key));
    } finally {
      parent.removeEventListener("abort", cancel);
      if (owns()) { mutation.current = null; update({ ...current.current, saving: false }); }
    }
  };
  const reload = () => {
    if (mutation.current || currentScope.current !== scope) return;
    blocked.current = true; generation.current?.abort();
    update({ ...current.current, loading: true, stale: true, errorKey: "", messageKey: "" });
    setReloadId(value => value + 1);
  };
  const dirty = !!view.draft && JSON.stringify(view.draft) !== JSON.stringify(view.settings);
  return { ...view, ready, dirty, edit, review, save, reload };
}
