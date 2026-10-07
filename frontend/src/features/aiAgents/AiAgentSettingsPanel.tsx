import { useRouter } from "next/router";
import { useConfirmedAction } from "../../components/useConfirmedAction";
import { useI18n } from "../../i18n/context";
import { useDraftWarning } from "../editor/useDraftWarning";
import { AI_AGENT_PROTOCOLS, MAX_AI_AGENT_PROFILES, type AiAgentProfile, type AiAgentProtocol } from "./aiAgentSettings";
import { useAiAgentSettings } from "./useAiAgentSettings";

const examples: Partial<Record<AiAgentProtocol, string>> = {
  openai_responses: "https://api.openai.com/v1", openai_chat_completions: "https://api.openai.com/v1",
  anthropic_messages: "https://api.anthropic.com", gemini_generate_content: "https://generativelanguage.googleapis.com/v1beta",
};

export default function AiAgentSettingsPanel({ engineUrl }: { engineUrl: string }) {
  // A reviewed configuration can never move to another Engine with the same component instance.
  return <AiAgentSettings key={engineUrl} engineUrl={engineUrl} />;
}

function AiAgentSettings({ engineUrl }: { engineUrl: string }) {
  const { t } = useI18n(), router = useRouter(), safety = useConfirmedAction();
  const form = useAiAgentSettings(engineUrl, router.asPath, t), disabled = !form.ready || safety.busy;
  useDraftWarning(form.dirty || form.saving, t("aiAgents.leaveConfirm"));
  const edit = (change: Parameters<typeof form.edit>[0]) => { if (!safety.isBusy()) form.edit(change); };
  const changeProfile = (index: number, change: Partial<AiAgentProfile>) => edit(draft => ({ ...draft,
    profiles: draft.profiles.map((profile, position) => position === index ? { ...profile, ...change } : profile) }));
  const save = () => {
    if (safety.isBusy()) return;
    const snapshot = form.review(); if (!snapshot) return;
    safety.request({ action: t("aiAgents.save"), description: t("aiAgents.saveReview"), targets: [engineUrl],
      details: [
        { label: t("aiAgents.revision"), value: String(snapshot.body.expected_revision) },
        { label: t("aiAgents.defaultProfile"), value: snapshot.body.default_profile_id ?? t("aiAgents.noDefault") },
        { label: t("aiAgents.profiles"), value: String(snapshot.body.profiles.length) },
        ...snapshot.body.profiles.map(profile => ({ label: `${profile.name} (${profile.id})`,
          value: [t(`aiAgents.protocols.${profile.protocol}`), profile.base_url, profile.model,
            profile.credential_ref ?? t("aiAgents.noReference"), t(profile.enabled ? "aiAgents.enabled" : "aiAgents.disabled")].join(" · ") })),
      ],
    }, signal => form.save(snapshot, signal));
  };
  const reload = () => {
    if (safety.isBusy()) return;
    if (form.dirty) safety.request({ action: t("aiAgents.reload"), description: t("aiAgents.reloadReview"),
      targets: [engineUrl], dangerous: false }, () => form.reload());
    else form.reload();
  };
  const add = () => edit(draft => {
    if (draft.profiles.length >= MAX_AI_AGENT_PROFILES) return draft;
    let sequence = 1; while (draft.profiles.some(profile => profile.id === `agent-${sequence}`)) sequence++;
    return { ...draft, profiles: [...draft.profiles, { id: `agent-${sequence}`, name: "", protocol: "openai_responses",
      base_url: "", model: "", credential_ref: null, enabled: false }] };
  });

  return <section className="panel" aria-label={t("aiAgents.title")} style={{ marginTop: 14 }}>
    {safety.dialog}
    <h3>{t("aiAgents.title")}</h3>
    <p className="meta">{t("aiAgents.subtitle")}</p>
    <p className="meta">{t("aiAgents.secretHint")}</p>
    {form.loading && <p role="status">{t("aiAgents.loading")}</p>}
    {form.errorKey && <p className="error" role="alert">{t(form.errorKey)}</p>}
    {form.stale && form.draft && <p role="status" className="meta">{t("aiAgents.stale")}</p>}
    {form.messageKey && <p role="status">{t(form.messageKey)}</p>}
    {form.draft && <>
      <div className="row" style={{ gap: 12, flexWrap: "wrap" }}>
        <label>{t("aiAgents.defaultProfile")} <select aria-label={t("aiAgents.defaultProfile")} disabled={disabled}
          value={form.draft.default_profile_id ?? ""} onChange={event => edit(draft => ({ ...draft, default_profile_id: event.target.value || null }))}>
          <option value="">{t("aiAgents.noDefault")}</option>
          {form.draft.profiles.filter(profile => profile.enabled).map((profile, index) =>
            <option key={index} value={profile.id}>{profile.name || profile.id}</option>)}
        </select></label>
        <span className="meta">{t("aiAgents.revision")}: {form.settings?.revision}</span>
      </div>
      {!form.draft.profiles.length && <p className="meta">{t("aiAgents.empty")}</p>}
      <div className="grid cols-2" style={{ marginTop: 12 }}>
        {form.draft.profiles.map((profile, index) => <fieldset key={index} className="panel" disabled={disabled}
          aria-label={t("aiAgents.profileNumber", { number: index + 1 })} style={{ minWidth: 0 }}>
          <legend>{t("aiAgents.profileNumber", { number: index + 1 })}</legend>
          <TextField label={t("aiAgents.id")} value={profile.id} onChange={id => edit(draft => ({ ...draft,
            default_profile_id: draft.default_profile_id === profile.id ? id : draft.default_profile_id,
            profiles: draft.profiles.map((item, position) => position === index ? { ...item, id } : item) }))} />
          <TextField label={t("aiAgents.name")} value={profile.name} onChange={name => changeProfile(index, { name })} />
          <label className="field-label">{t("aiAgents.protocol")}<select aria-label={t("aiAgents.protocol")} value={profile.protocol}
            onChange={event => changeProfile(index, { protocol: event.target.value as AiAgentProtocol })}>
            {AI_AGENT_PROTOCOLS.map(protocol => <option key={protocol} value={protocol}>{t(`aiAgents.protocols.${protocol}`)}</option>)}
          </select></label>
          <TextField label={t("aiAgents.baseUrl")} value={profile.base_url} onChange={base_url => changeProfile(index, { base_url })} />
          <p className="meta">{t("aiAgents.endpointHint")}</p>
          {examples[profile.protocol] && <button type="button" onClick={() => changeProfile(index, { base_url: examples[profile.protocol] })}>
            {t("aiAgents.useExample")}
          </button>}
          <TextField label={t("aiAgents.model")} value={profile.model} placeholder={t("aiAgents.modelPlaceholder")}
            onChange={model => changeProfile(index, { model })} />
          <TextField label={t("aiAgents.credentialRef")} value={profile.credential_ref ?? ""} placeholder="TEAM_AI_KEY"
            onChange={credential_ref => changeProfile(index, { credential_ref: credential_ref || null })} />
          <label><input type="checkbox" checked={profile.enabled} onChange={event => edit(draft => ({ ...draft,
            default_profile_id: !event.target.checked && draft.default_profile_id === profile.id ? null : draft.default_profile_id,
            profiles: draft.profiles.map((item, position) => position === index ? { ...item, enabled: event.target.checked } : item) }))} /> {t("aiAgents.enabled")}</label>
          <button type="button" className="button-danger" style={{ marginLeft: 12 }} onClick={() => edit(draft => ({ ...draft,
            default_profile_id: draft.default_profile_id === profile.id ? null : draft.default_profile_id,
            profiles: draft.profiles.filter((_item, position) => position !== index) }))}>{t("aiAgents.remove")}</button>
        </fieldset>)}
      </div>
    </>}
    <div className="row" style={{ gap: 8, marginTop: 12, flexWrap: "wrap" }}>
      <button type="button" onClick={add} disabled={disabled || (form.draft?.profiles.length ?? 0) >= MAX_AI_AGENT_PROFILES}>{t("aiAgents.add")}</button>
      <button type="button" onClick={save} disabled={disabled || !form.dirty}>{t("aiAgents.save")}</button>
      <button type="button" onClick={reload} disabled={form.loading || form.saving || safety.busy}>{t("aiAgents.reload")}</button>
      {form.dirty && <span className="meta">{t("aiAgents.unsaved")}</span>}
    </div>
  </section>;
}

function TextField({ label, value, onChange, placeholder }: {
  label: string; value: string; onChange: (value: string) => void; placeholder?: string;
}) {
  return <label className="field-label" style={{ display: "grid", gap: 4, marginBottom: 10 }}>{label}
    <input type="text" aria-label={label} autoComplete="off" spellCheck={false} value={value} placeholder={placeholder}
      onChange={event => onChange(event.target.value)} />
  </label>;
}
