import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/router";
import { useI18n } from "../i18n/context";

type Confirmation = {
  action: string;
  description: string;
  targets?: string[];
  details?: { label: string; value: string }[];
  phrase?: string;
  preview?: string;
  dangerous?: boolean;
};
type PendingAction = Confirmation & {
  execute: (signal: AbortSignal) => void | Promise<void>;
  controller: AbortController;
  trigger: HTMLElement | null;
};

// Capture the reviewed action/targets once. A ref closes the gap before React disables controls.
export function useConfirmedAction() {
  const [pending, setPending] = useState<PendingAction | null>(null);
  const current = useRef<PendingAction | null>(null);
  const mounted = useRef(true);
  const router = useRouter();
  const close = useCallback(() => {
    current.current?.controller.abort();
    current.current = null;
    if (mounted.current) setPending(null);
  }, []);
  useEffect(() => { close(); }, [router.asPath, close]);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; current.current?.controller.abort(); current.current = null; };
  }, []);
  const request = (options: Confirmation, execute: PendingAction["execute"]) => {
    if (!mounted.current || current.current) return;
    const action = { ...options, execute, controller: new AbortController(),
      trigger: document.activeElement instanceof HTMLElement ? document.activeElement : null };
    current.current = action;
    setPending(action);
  };
  return {
    request,
    busy: pending !== null,
    isBusy: () => current.current !== null,
    dialog: pending ? <ConfirmationDialog action={pending} onClose={close} /> : null,
  };
}

function ConfirmationDialog({ action, onClose }: { action: PendingAction; onClose: () => void }) {
  const { t } = useI18n();
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const executing = useRef(false);
  const mounted = useRef(true);
  const [phase, setPhase] = useState<"confirm" | "running" | "failed">("confirm");
  const [phrase, setPhrase] = useState("");
  const [error, setError] = useState("");
  useEffect(() => { if (phase === "failed") cancel.current?.focus(); }, [phase]);
  useEffect(() => {
    const trigger = action.trigger;
    const element = dialog.current;
    element?.showModal();
    cancel.current?.focus();
    mounted.current = true;
    return () => {
      mounted.current = false;
      element?.close();
      // Strict Mode replays visual effect cleanup; only explicit dismissal/navigation
      // may discard the pending action. The owning hook handles real unmounts.
      if (trigger?.isConnected) trigger.focus({ preventScroll: true });
    };
  }, [action, onClose]);

  const execute = async () => {
    if (executing.current || action.controller.signal.aborted || phase !== "confirm" || (action.phrase && phrase !== action.phrase)) return;
    executing.current = true;
    setPhase("running");
    try {
      await action.execute(action.controller.signal);
      if (mounted.current) onClose();
    } catch (cause) {
      if (mounted.current) {
        setError(cause instanceof Error ? cause.message : String(cause));
        setPhase("failed");
      }
    }
    // No implicit retry: after failure the user must inspect state and open a fresh confirmation.
  };

  return <dialog ref={dialog} className="safety-dialog" aria-labelledby="safety-dialog-title"
    aria-describedby="safety-dialog-description" onCancel={event => {
      event.preventDefault();
      if (!executing.current || phase === "failed") onClose();
    }}>
    <p className="brand-kicker">{action.action}</p>
    <h2 id="safety-dialog-title">{t("safety.title")}</h2>
    <p id="safety-dialog-description">{action.description}</p>
    {action.targets && <div className="safety-targets">
      <strong>{t("safety.targets", { count: action.targets.length })}</strong>
      <ul>{action.targets.map((target, index) => <li key={index}><code>{target}</code></li>)}</ul>
    </div>}
    {action.details && <dl className="safety-details">{action.details.map((detail, index) => <div key={index}>
      <dt>{detail.label}</dt><dd>{detail.value}</dd>
    </div>)}</dl>}
    {action.preview !== undefined && <details><summary>{t("ebpf.source")}</summary><pre>{action.preview}</pre></details>}
    {action.phrase && phase === "confirm" && <label className="field-label safety-phrase">
      <span>{t("safety.typePhrase", { phrase: action.phrase })}</span>
      <input aria-label={t("safety.phrase")} autoComplete="off" spellCheck={false} value={phrase}
        onChange={event => setPhrase(event.target.value)} />
    </label>}
    {phase === "running" && <p role="status">{t("safety.running")}</p>}
    {phase === "failed" && <div className="error" role="alert"><p>{t("safety.failed")}</p><p>{error}</p></div>}
    <div className="safety-actions">
      <button ref={cancel} type="button" className="button-secondary" disabled={phase === "running"} onClick={onClose}>
        {phase === "failed" ? t("safety.close") : t("safety.cancel")}
      </button>
      {phase !== "failed" && <button type="button" className={action.dangerous === false ? "button-primary" : "button-danger"}
        disabled={phase === "running" || Boolean(action.phrase && phrase !== action.phrase)} onClick={() => void execute()}>
        {t("safety.confirm", { action: action.action })}
      </button>}
    </div>
  </dialog>;
}
