import Link from "next/link";
import { useEffect, useState } from "react";

import { useI18n } from "../../i18n/context";
import { AttemptCard } from "./AttemptCard";
import type { LabAttempt } from "./models";

export function AttemptHistory({ engineUrl }: { engineUrl: string }) {
  const { t } = useI18n();
  const [attempts, setAttempts] = useState<LabAttempt[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const [visibleCount, setVisibleCount] = useState(20);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError("");
    const load = async () => {
      try {
        const response = await fetch(`${engineUrl}/learning/attempts`, { credentials: "include", signal: controller.signal });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const payload = await response.json() as LabAttempt[];
        if (!controller.signal.aborted) setAttempts(payload);
      } catch (cause) {
        if (!controller.signal.aborted) setError(`${t("learn.historyLoadFailed")}: ${(cause as Error).message}`);
      } finally { if (!controller.signal.aborted) setLoading(false); }
    };
    void load();
    return () => controller.abort();
  }, [engineUrl, refresh, t]);

  return (
    <section className="panel" style={{ marginTop: 16 }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <h2>{t("learn.historyTitle")}</h2>
        <button type="button" disabled={loading} onClick={() => setRefresh((value) => value + 1)}>
          {loading ? t("common.checking") : t("common.refresh")}
        </button>
      </div>
      <p className="meta">{t("learn.historyHint")}</p>
      {error && <p className="error" role="alert">{error}</p>}
      {!loading && !error && attempts.length === 0 && <p className="meta">{t("learn.historyEmpty")}</p>}
      <div className="grid" style={{ gap: 16 }}>
        {attempts.slice(0, visibleCount).map((attempt) => <div key={attempt.id}>
          <AttemptCard attempt={attempt} />
          <p><Link href={`/ebpf?lab=${encodeURIComponent(attempt.lab_id)}`}>{t("learn.openEditor")}</Link></p>
        </div>)}
      </div>
      {visibleCount < attempts.length && <button type="button" style={{ marginTop: 16 }}
        onClick={() => setVisibleCount((value) => value + 20)}>{t("learn.historyMore")}</button>}
    </section>
  );
}
