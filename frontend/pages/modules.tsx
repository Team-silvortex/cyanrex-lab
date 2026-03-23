import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

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
  const [state, setState] = useState<HeaderState | null>(null);
  const [loading, setLoading] = useState(false);
  const [batching, setBatching] = useState(false);
  const [message, setMessage] = useState<string | null>(() =>
    loadPageState<string>("modules_message_v1"),
  );
  const [progress, setProgress] = useState<{
    label: string;
    total: number;
    done: number;
  } | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const refresh = async () => {
    setLoading(true);
    try {
      const response = await fetch(`${engineUrl}/modules/c-headers/catalog`, {
        credentials: "include",
      });
      const json = (await response.json()) as HeaderState;
      setState(json);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    savePageState("modules_message_v1", message ?? "");
  }, [message]);

  const download = async (id: string) => {
    setMessage(null);
    const response = await fetch(`${engineUrl}/modules/c-headers/download`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ id }),
    });

    const json = (await response.json()) as ActionResponse;
    setMessage(json.message);
    await refresh();
  };

  const toggle = async (id: string, selected: boolean) => {
    setMessage(null);
    const response = await fetch(`${engineUrl}/modules/c-headers/select`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ id, selected }),
    });

    const json = (await response.json()) as ActionResponse;
    setMessage(json.message);
    await refresh();
  };

  const deleteOne = async (id: string) => {
    setMessage(null);
    const response = await fetch(`${engineUrl}/modules/c-headers/delete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ id }),
    });

    const json = (await response.json()) as ActionResponse;
    setMessage(json.message);
    await refresh();
  };

  const batchToggle = async (selected: boolean, onlyDownloaded: boolean) => {
    if (!state?.headers?.length) return;

    setBatching(true);
    setMessage(null);

    const targets = state.headers.filter((header) =>
      onlyDownloaded ? header.downloaded : true,
    );

    try {
      setProgress({
        label: selected ? t("modules.progressSelecting") : t("modules.progressUnselecting"),
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/select`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ id: header.id, selected }),
        });
        setProgress((prev) =>
          prev
            ? {
                ...prev,
                done: idx + 1,
              }
            : prev,
        );
      }

      setState((prev) =>
        prev
          ? {
              headers: prev.headers.map((header) =>
                targets.some((t) => t.id === header.id)
                  ? { ...header, selected }
                  : header,
              ),
            }
          : prev,
      );

      setMessage(
        selected
          ? onlyDownloaded
            ? t("modules.selectedAllDownloaded")
            : t("modules.selectedAll")
          : t("modules.unselectedAll"),
      );
    } finally {
      setProgress(null);
      setBatching(false);
      await refresh();
    }
  };

  const batchDownloadSelected = async () => {
    if (!state?.headers?.length) return;
    setBatching(true);
    setMessage(null);

    const targets = state.headers.filter((header) => header.selected);
    try {
      setProgress({
        label: t("modules.progressDownloading"),
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/download`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ id: header.id }),
        });
        setProgress((prev) =>
          prev
            ? {
                ...prev,
                done: idx + 1,
              }
            : prev,
        );
      }
      setMessage(t("modules.downloadedCount", { count: targets.length }));
    } finally {
      setProgress(null);
      setBatching(false);
      await refresh();
    }
  };

  const batchDeleteSelected = async () => {
    if (!state?.headers?.length) return;
    setBatching(true);
    setMessage(null);

    const targets = state.headers.filter((header) => header.selected);
    try {
      setProgress({
        label: t("modules.progressDeleting"),
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/delete`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ id: header.id }),
        });
        setProgress((prev) =>
          prev
            ? {
                ...prev,
                done: idx + 1,
              }
            : prev,
        );
      }
      setMessage(t("modules.deletedCount", { count: targets.length }));
    } finally {
      setProgress(null);
      setBatching(false);
      await refresh();
    }
  };

  return (
    <SidebarLayout title={t("layout.nav.modules")}>
      <section className="panel">
        <h2>{t("modules.title")}</h2>
        <p className="meta">
          {t("modules.subtitle")}
        </p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={refresh} disabled={loading}>
            {loading ? t("modules.refreshing") : t("modules.refreshCatalog")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, false)}
            disabled={batching || loading}
          >
            {t("modules.selectAll")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, true)}
            disabled={batching || loading}
          >
            {t("modules.selectDownloaded")}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(false, false)}
            disabled={batching || loading}
          >
            {t("modules.unselectAll")}
          </button>
          <button
            type="button"
            onClick={batchDownloadSelected}
            disabled={batching || loading}
          >
            {t("modules.downloadSelected")}
          </button>
          <button
            type="button"
            onClick={batchDeleteSelected}
            disabled={batching || loading}
          >
            {t("modules.deleteSelected")}
          </button>
        </div>

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
              <button type="button" onClick={() => download(header.id)}>
                {t("modules.download")}
              </button>
              <button type="button" onClick={() => deleteOne(header.id)}>
                {t("modules.delete")}
              </button>
              <label className="meta" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <input
                  type="checkbox"
                  checked={header.selected}
                  onChange={(event) => toggle(header.id, event.target.checked)}
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
