import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

type EventOverflowPolicy = "drop_oldest" | "drop_new";

type EventSettingsResponse = {
  max_records: number;
  overflow_policy: EventOverflowPolicy;
};

type UpdateEventSettingsResponse = {
  ok: boolean;
  message: string;
  settings?: EventSettingsResponse;
};

export default function SettingsPage() {
  const { t } = useI18n();
  const [maxRecords, setMaxRecords] = useState(
    () => loadPageState<number>("settings_event_max_records_v1") ?? 500,
  );
  const [overflowPolicy, setOverflowPolicy] = useState<EventOverflowPolicy>(
    () => loadPageState<EventOverflowPolicy>("settings_event_overflow_policy_v1") ?? "drop_oldest",
  );
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  useEffect(() => {
    savePageState("settings_event_max_records_v1", maxRecords);
    savePageState("settings_event_overflow_policy_v1", overflowPolicy);
  }, [maxRecords, overflowPolicy]);

  useEffect(() => {
    let active = true;
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const response = await fetch(`${engineUrl}/settings/events`, {
          credentials: "include",
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const json = (await response.json()) as EventSettingsResponse;
        if (!active) return;
        setMaxRecords(json.max_records);
        setOverflowPolicy(json.overflow_policy);
      } catch (err) {
        if (!active) return;
        setError((err as Error).message);
      } finally {
        if (active) setLoading(false);
      }
    };
    load();
    return () => {
      active = false;
    };
  }, [engineUrl]);

  const save = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
    try {
      const response = await fetch(`${engineUrl}/settings/events`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          max_records: Math.max(50, Math.min(50000, Number(maxRecords) || 500)),
          overflow_policy: overflowPolicy,
        }),
      });
      const json = (await response.json()) as UpdateEventSettingsResponse;
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }
      if (json.settings) {
        setMaxRecords(json.settings.max_records);
        setOverflowPolicy(json.settings.overflow_policy);
      }
      setMessage(t("settings.saved"));
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <SidebarLayout title={t("settings.title")}>
      <section className="panel">
        <h2>{t("settings.title")}</h2>
        <p className="meta">{t("settings.subtitle")}</p>

        <div className="grid cols-2" style={{ marginTop: 12 }}>
          <label className="meta">
            {t("settings.maxRecords")}
            <input
              type="number"
              min={50}
              max={50000}
              value={maxRecords}
              onChange={(event) => setMaxRecords(Number(event.target.value) || 500)}
              style={{ marginTop: 6, width: "100%" }}
            />
          </label>

          <label className="meta">
            {t("settings.overflowPolicy")}
            <select
              value={overflowPolicy}
              onChange={(event) => setOverflowPolicy(event.target.value as EventOverflowPolicy)}
              style={{ marginTop: 6, width: "100%" }}
            >
              <option value="drop_oldest">{t("settings.dropOldest")}</option>
              <option value="drop_new">{t("settings.dropNew")}</option>
            </select>
          </label>
        </div>

        <p className="meta" style={{ marginTop: 10 }}>
          {overflowPolicy === "drop_oldest" ? t("settings.dropOldestHint") : t("settings.dropNewHint")}
        </p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={save} disabled={saving || loading}>
            {saving ? t("settings.saving") : t("settings.save")}
          </button>
          {loading && <span className="meta">{t("settings.loading")}</span>}
        </div>

        {message && <p className="meta" style={{ color: "#9cd67a" }}>{message}</p>}
        {error && <p className="error">{error}</p>}
      </section>
    </SidebarLayout>
  );
}
