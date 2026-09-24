import { useEffect, useRef, useState } from "react";
import { loadPageState, savePageState } from "../../utils/pageState";
import { notifyUnreadChanged } from "../events/eventRequest";
import { normalizeEventDraft, parseCompilerSettings, parseCompilerSettingsSaved, parseEventSettings,
  parseEventSettingsSaved, requestSettings, type CompilerSettings, type EventSettings } from "./settingsRequest";

type FormState = {
  scope: string; draft: EventSettings; resident: boolean;
  events: EventSettings | null; compiler: CompilerSettings | null;
  loading: boolean; saving: boolean; verify: boolean; errorKey: string; messageKey: string;
};
type Review = { scope: string; generation: AbortController; events: EventSettings; compiler: { resident: boolean } | null };
const MAX_KEY = "settings_event_max_records_v1", POLICY_KEY = "settings_event_overflow_policy_v1";
function initial(scope: string): FormState {
  let draft: EventSettings = { max_records: 500, overflow_policy: "drop_oldest" };
  try { draft = parseEventSettings({ max_records: loadPageState(MAX_KEY), overflow_policy: loadPageState(POLICY_KEY) }); } catch { /* Untrusted drafts never establish server state. */ }
  return { scope, draft, resident: false, events: null, compiler: null, loading: true, saving: false, verify: false, errorKey: "", messageKey: "" };
}

export function useSettingsForm(engineUrl: string, navigation: string, t: (key: string) => string) {
  const scope = JSON.stringify([engineUrl, navigation]), currentScope = useRef(scope); currentScope.current = scope;
  const generation = useRef<AbortController | null>(null), mutation = useRef<AbortController | null>(null), blocked = useRef(true);
  const [revision, setRevision] = useState(0), [state, setState] = useState(() => initial(scope));
  const view = state.scope === scope ? state : initial(scope);

  useEffect(() => {
    const controller = new AbortController(); generation.current = controller; blocked.current = true;
    setState(initial(scope));
    const owns = () => generation.current === controller && currentScope.current === scope && !controller.signal.aborted;
    void (async () => {
      const [events, compiler] = await Promise.allSettled([
        requestSettings(`${engineUrl}/settings/events`, controller.signal, parseEventSettings),
        requestSettings(`${engineUrl}/settings/compiler`, controller.signal, parseCompilerSettings),
      ]);
      if (!owns()) return;
      blocked.current = events.status !== "fulfilled";
      setState(previous => ({ ...previous, scope, loading: false,
        events: events.status === "fulfilled" ? events.value : null,
        draft: events.status === "fulfilled" ? events.value : previous.draft,
        compiler: compiler.status === "fulfilled" ? compiler.value : null,
        resident: compiler.status === "fulfilled" ? compiler.value.resident : false,
        errorKey: events.status === "fulfilled" ? "" : "settings.loadFailed" }));
    })();
    return () => {
      controller.abort();
      if (generation.current === controller) {
        generation.current = null; blocked.current = true;
        mutation.current?.abort(); mutation.current = null;
      }
    };
  }, [engineUrl, scope, revision]);

  useEffect(() => {
    if (state.scope !== scope) return;
    savePageState(MAX_KEY, state.draft.max_records); savePageState(POLICY_KEY, state.draft.overflow_policy);
  }, [scope, state.scope, state.draft]);

  const ready = state.scope === scope && !!view.events && !view.loading && !view.saving && !view.verify;
  const edit = (change: Partial<Pick<FormState, "draft" | "resident">>) => {
    if (!ready || blocked.current || currentScope.current !== scope) return;
    setState(previous => ({ ...previous, ...change, errorKey: "", messageKey: "" }));
  };
  const review = (): Review | null => {
    if (!ready || blocked.current || currentScope.current !== scope || !generation.current) return null;
    try {
      return { scope, generation: generation.current, events: normalizeEventDraft(view.draft),
        compiler: view.compiler ? { resident: view.resident } : null };
    } catch { setState(previous => ({ ...previous, errorKey: "settings.invalidDraft", messageKey: "" })); return null; }
  };
  const save = async (snapshot: Review, parent: AbortSignal) => {
    parent.throwIfAborted();
    if (blocked.current || mutation.current || snapshot.generation !== generation.current || snapshot.scope !== currentScope.current) {
      throw new Error(t("settings.verifyBeforeSave"));
    }
    const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
    parent.addEventListener("abort", cancel, { once: true }); mutation.current = controller; blocked.current = true;
    const owns = () => generation.current === snapshot.generation && currentScope.current === snapshot.scope && mutation.current === controller;
    setState(previous => ({ ...previous, saving: true, errorKey: "", messageKey: "" }));
    let eventsSaved = false;
    try {
      const events = await requestSettings(`${engineUrl}/settings/events`, controller.signal,
        value => parseEventSettingsSaved(value, snapshot.events), snapshot.events);
      controller.signal.throwIfAborted();
      if (!owns()) return;
      eventsSaved = true;
      setState(previous => ({ ...previous, events, draft: events }));
      notifyUnreadChanged(engineUrl);
      let compiler: CompilerSettings | null = null;
      if (snapshot.compiler) {
        const resident = snapshot.compiler.resident;
        compiler = await requestSettings(`${engineUrl}/settings/compiler`, controller.signal,
          value => parseCompilerSettingsSaved(value, resident), { resident });
      }
      controller.signal.throwIfAborted();
      if (owns()) {
        blocked.current = false;
        setState(previous => ({ ...previous, compiler, resident: compiler?.resident ?? previous.resident,
          messageKey: compiler ? "settings.saved" : "settings.eventsOnlySaved" }));
      }
    } catch {
      const key = eventsSaved ? "settings.partialSave" : "settings.saveUnconfirmed";
      if (owns()) setState(previous => ({ ...previous, verify: true, errorKey: key, messageKey: "" }));
      throw new Error(t(key));
    } finally {
      parent.removeEventListener("abort", cancel);
      if (owns()) { mutation.current = null; setState(previous => ({ ...previous, saving: false })); }
    }
  };
  const reload = () => {
    if (mutation.current || currentScope.current !== scope) return;
    blocked.current = true; generation.current?.abort();
    setState(previous => ({ ...previous, loading: true, errorKey: "", messageKey: "" }));
    setRevision(value => value + 1);
  };
  const dirty = !!view.events && (view.draft.max_records !== view.events.max_records
    || view.draft.overflow_policy !== view.events.overflow_policy
    || (!!view.compiler && view.resident !== view.compiler.resident));
  return { ...view, ready, dirty, review, save, reload,
    changeEvents: (draft: EventSettings) => edit({ draft }), changeResident: (resident: boolean) => edit({ resident }) };
}
