import { useEffect, useRef, useState } from "react";

import { useConfirmedAction } from "../src/components/useConfirmedAction";
import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";
import { canManageDeployment } from "../src/utils/sidebarPermissions";

type HeaderItem = {
  id: string;
  name: string;
  description: string;
  source_url: string;
  downloaded: boolean;
  selected: boolean;
  local_path: string;
};

type HeaderState = {
  headers: HeaderItem[];
};

type ActionResponse = {
  ok: boolean;
  message: string;
};

export default function ModulesPage() {
  const { t } = useI18n();
  const safety = useConfirmedAction();
  const operationInFlight = useRef(false);
  const [error, setError] = useState("");
  const [state, setState] = useState<HeaderState | null>(null);
  const [loading, setLoading] = useState(false);
  const [batching, setBatching] = useState(false);
  const [message, setMessage] = useState<string | null>(() =>
    loadPageState<string>("modules_message_v1"),
  );
  const [canManageModules, setCanManageModules] = useState(false);
  const [roleReady, setRoleReady] = useState(false);
  const [progress, setProgress] = useState<{
    label: string;
    total: number;
    done: number;
  } | null>(null);

  const engineUrl = getEngineUrl();

  const refresh = async () => {
    setLoading(true);
    try {
      const response = await fetch(`${engineUrl}/modules/c-headers/catalog`, {
        credentials: "include",
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const json = (await response.json()) as HeaderState;
      setState(json);
    } finally {
      setLoading(false);
    }
  };

  const refreshRole = async () => {
    try {
      const response = await fetch(`${engineUrl}/auth/me`, {
        credentials: "include",
      });
      if (!response.ok) {
        setCanManageModules(false);
        return;
      }
      const json = (await response.json()) as { authenticated?: boolean; role?: string };
      setCanManageModules(json.authenticated === true && canManageDeployment(json.role));
    } catch { setCanManageModules(false); } finally {
      setRoleReady(true);
    }
  };

  useEffect(() => {
    void refresh().catch(cause => setError((cause as Error).message));
    void refreshRole();
  }, []);

  useEffect(() => {
    savePageState("modules_message_v1", message ?? "");
  }, [message]);

  const performHeaders = async (
    operation: "download" | "select" | "delete", targets: HeaderItem[], label: string, success: string, selected?: boolean,
  ) => {
    if (!canManageModules || operationInFlight.current || !targets.length) return;
    operationInFlight.current = true;
    setBatching(true);
    setMessage(null);
    setError("");
    let done = 0;
    setProgress({ label, total: targets.length, done });
    try {
      for (const header of targets) {
        const response = await fetch(`${engineUrl}/modules/c-headers/${operation}`, {
          method: "POST", headers: { "Content-Type": "application/json" }, credentials: "include",
          body: JSON.stringify({ id: header.id, ...(selected === undefined ? {} : { selected }) }),
        });
        const payload = await response.json() as ActionResponse;
        if (!response.ok || !payload.ok) throw new Error(payload.message || `HTTP ${response.status}`);
        done += 1;
        setProgress({ label, total: targets.length, done });
      }
      setMessage(success);
    } catch (cause) {
      const failure = `${t("safety.partial", { done, total: targets.length })} ${(cause as Error).message}`;
      setError(failure);
      throw new Error(failure);
    } finally {
      try { await refresh(); } catch (cause) { setError((cause as Error).message); }
      setProgress(null);
      setBatching(false);
      operationInFlight.current = false;
    }
  };

  const download = (id: string) => {
    if (safety.isBusy()) return;
    const targets = state?.headers.filter(header => header.id === id) ?? [];
    void performHeaders("download", targets, t("modules.progressDownloading"), t("modules.downloadedCount", { count: targets.length })).catch(() => {});
  };
  const toggle = (id: string, selected: boolean) => {
    if (safety.isBusy()) return;
    const targets = state?.headers.filter(header => header.id === id) ?? [];
    void performHeaders("select", targets, t("modules.progressSelecting"), "", selected).catch(() => {});
  };
  const batchToggle = (selected: boolean, onlyDownloaded: boolean) => {
    if (safety.isBusy()) return;
    const targets = state?.headers.filter(header => !onlyDownloaded || header.downloaded) ?? [];
    void performHeaders("select", targets, t(selected ? "modules.progressSelecting" : "modules.progressUnselecting"),
      t(selected ? onlyDownloaded ? "modules.selectedAllDownloaded" : "modules.selectedAll" : "modules.unselectedAll"), selected).catch(() => {});
  };
  const batchDownloadSelected = () => {
    if (safety.isBusy()) return;
    const targets = state?.headers.filter(header => header.selected) ?? [];
    void performHeaders("download", targets, t("modules.progressDownloading"), t("modules.downloadedCount", { count: targets.length })).catch(() => {});
  };
  const confirmDelete = (targets: HeaderItem[], batch = false) => {
    if (!canManageModules || operationInFlight.current || !targets.length) return;
    safety.request({ action: t(batch ? "modules.deleteSelected" : "modules.delete"), description: t("safety.deleteHeaders"),
      targets: targets.map(header => header.id), phrase: batch ? "DELETE" : undefined },
      () => performHeaders("delete", targets, t("modules.progressDeleting"), t("modules.deletedCount", { count: targets.length })));
  };
  const deleteOne = (id: string) => confirmDelete(state?.headers.filter(header => header.id === id) ?? []);
  const batchDeleteSelected = () => confirmDelete(state?.headers.filter(header => header.selected) ?? [], true);

  return (
    <SidebarLayout title={t("layout.nav.modules")}>
      {safety.dialog}
      <section className="panel">
        <h2>{t("modules.title")}</h2>
        <p className="meta">
          {t("modules.subtitle")}
        </p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={() => void refresh().catch(cause => setError((cause as Error).message))} disabled={loading || batching || safety.busy || !roleReady}>
            {loading ? t("modules.refreshing") : t("modules.refreshCatalog")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, false)}
            disabled={safety.busy || batching || loading || !canManageModules}
          >
            {t("modules.selectAll")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, true)}
            disabled={safety.busy || batching || loading || !canManageModules}
          >
            {t("modules.selectDownloaded")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(false, false)}
            disabled={safety.busy || batching || loading || !canManageModules}
          >
            {t("modules.unselectAll")}
          </button>
          <button
            type="button"
            onClick={batchDownloadSelected}
            disabled={safety.busy || batching || loading || !canManageModules}
          >
            {t("modules.downloadSelected")}
          </button>
          <button
            type="button"
            className="button-danger"
            onClick={batchDeleteSelected}
            disabled={safety.busy || batching || loading || !canManageModules}
          >
            {t("modules.deleteSelected")}
          </button>
        </div>

        {!roleReady ? null : canManageModules ? null : (
          <p className="meta" style={{ marginTop: 10 }}>
            {t("modules.teacherReadonlyTip")}
          </p>
        )}

        {error && <p className="error" role="alert">{error}</p>}
        {message && <p className="meta" style={{ marginTop: 10 }}>{message}</p>}
        {progress && (
          <div className="panel" style={{ marginTop: 10, background: "#0b1425" }}>
            <p className="meta" style={{ marginTop: 0 }}>
              {progress.label} {progress.done}/{progress.total}
            </p>
            <div
              style={{
                width: "100%",
                height: 10,
                border: "1px solid #1d2f4f",
                borderRadius: 999,
                overflow: "hidden",
                background: "#071022",
              }}
            >
              <div
                style={{
                  width:
                    progress.total === 0
                      ? "100%"
                      : `${Math.round((progress.done / progress.total) * 100)}%`,
                  height: "100%",
                  background: "linear-gradient(90deg, #2d63bf, #5fa8ff)",
                  transition: "width 0.15s ease",
                }}
              />
            </div>
          </div>
        )}

        {state?.headers?.map((header) => (
          <article key={header.id} className="panel" style={{ marginTop: 12, background: "#0b1425" }}>
            <p><strong>{header.name}</strong></p>
            <p className="meta">{header.description}</p>
            <p className="meta">{t("modules.source")}: {header.source_url}</p>
            <p className="meta">{t("modules.local")}: {header.local_path}</p>

            <div className="row" style={{ marginTop: 8 }}>
              <button
                type="button"
                onClick={() => download(header.id)}
                disabled={safety.busy || batching || loading || !canManageModules}
              >
                {t("modules.download")}
              </button>
              <button
                type="button"
                className="button-danger"
                onClick={() => deleteOne(header.id)}
                disabled={safety.busy || batching || loading || !canManageModules}
              >
                {t("modules.delete")}
              </button>
              <label className="meta" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <input
                  type="checkbox"
                  checked={header.selected}
                  onChange={(event) => toggle(header.id, event.target.checked)}
                  disabled={safety.busy || batching || loading || !canManageModules}
                />
                {t("modules.injectMetadata")}
              </label>
              <span className="meta">{t("modules.downloaded")}: {String(header.downloaded)}</span>
            </div>
          </article>
        ))}
      </section>
    </SidebarLayout>
  );
}
