import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";

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
  const [state, setState] = useState<HeaderState | null>(null);
  const [loading, setLoading] = useState(false);
  const [batching, setBatching] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
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
      const response = await fetch(`${engineUrl}/modules/c-headers/catalog`);
      const json = (await response.json()) as HeaderState;
      setState(json);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const download = async (id: string) => {
    setMessage(null);
    const response = await fetch(`${engineUrl}/modules/c-headers/download`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
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
        label: selected ? "正在批量勾选..." : "正在批量取消...",
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/select`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
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
            ? "已全选（仅已下载头文件）"
            : "已全选（全部头文件）"
          : "已全部取消选择",
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
        label: "正在下载已勾选头文件...",
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/download`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
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
      setMessage(`已下载 ${targets.length} 个已勾选头文件`);
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
        label: "正在删除已勾选头文件...",
        total: targets.length,
        done: 0,
      });

      for (let idx = 0; idx < targets.length; idx += 1) {
        const header = targets[idx];
        await fetch(`${engineUrl}/modules/c-headers/delete`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
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
      setMessage(`已删除 ${targets.length} 个已勾选头文件`);
    } finally {
      setProgress(null);
      setBatching(false);
      await refresh();
    }
  };

  return (
    <SidebarLayout title="Cyanrex Modules">
      <section className="panel">
        <h2>C Header Module</h2>
        <p className="meta">
          下载常用 C/eBPF 头文件到本地，并勾选注入编辑器 metadata/诊断。
        </p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={refresh} disabled={loading}>
            {loading ? "Refreshing..." : "Refresh Catalog"}
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, false)}
            disabled={batching || loading}
          >
            全选（全部）
          </button>
          <button
            type="button"
            onClick={() => batchToggle(true, true)}
            disabled={batching || loading}
          >
            全选（已下载）
          </button>
          <button
            type="button"
            onClick={() => batchToggle(false, false)}
            disabled={batching || loading}
          >
            全部取消
          </button>
          <button
            type="button"
            onClick={batchDownloadSelected}
            disabled={batching || loading}
          >
            下载已勾选
          </button>
          <button
            type="button"
            onClick={batchDeleteSelected}
            disabled={batching || loading}
          >
            删除已勾选
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
            <p className="meta">source: {header.source_url}</p>
            <p className="meta">local: {header.local_path}</p>

            <div className="row" style={{ marginTop: 8 }}>
              <button type="button" onClick={() => download(header.id)}>
                Download
              </button>
              <button type="button" onClick={() => deleteOne(header.id)}>
                Delete
              </button>
              <label className="meta" style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <input
                  type="checkbox"
                  checked={header.selected}
                  onChange={(event) => toggle(header.id, event.target.checked)}
                />
                Inject to editor metadata
              </label>
              <span className="meta">downloaded: {String(header.downloaded)}</span>
            </div>
          </article>
        ))}
      </section>
    </SidebarLayout>
  );
}
