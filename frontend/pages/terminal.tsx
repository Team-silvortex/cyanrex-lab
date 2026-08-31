import Link from "next/link";
import { FormEvent, useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import {
  buildCommandRequest,
  commandNeedsModuleName,
  type CommandRequest,
  type CommandResponse,
  type CommandType,
  type ModuleInfo,
  summarizeCommandResponse,
} from "../src/features/terminal/command";
import { useI18n } from "../src/i18n/context";

type CommandHistoryEntry = {
  id: number;
  requestedAt: string;
  request: CommandRequest;
  response?: CommandResponse;
  error?: string;
};

export default function TerminalPage() {
  const { t } = useI18n();
  const engineUrl = useMemo(getEngineUrl, []);
  const [commandType, setCommandType] = useState<CommandType>("ListModules");
  const [moduleName, setModuleName] = useState("");
  const [modules, setModules] = useState<ModuleInfo[]>([]);
  const [history, setHistory] = useState<CommandHistoryEntry[]>([]);
  const [lastResponse, setLastResponse] = useState<CommandResponse | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const loadCatalog = async () => {
      try {
        const response = await fetch(`${engineUrl}/modules`, { credentials: "include" });
        if (!response.ok) return;
        const catalog = (await response.json()) as ModuleInfo[];
        if (!cancelled) setModules(catalog);
      } catch {
        // Command execution still reports actionable errors if the initial catalog is unavailable.
      }
    };
    loadCatalog();
    return () => {
      cancelled = true;
    };
  }, [engineUrl]);

  const runCommand = async (event: FormEvent) => {
    event.preventDefault();
    setError("");

    let request: CommandRequest;
    try {
      request = buildCommandRequest(commandType, moduleName);
    } catch {
      setError(t("terminal.moduleNameRequired"));
      return;
    }

    setLoading(true);
    const requestedAt = new Date().toISOString();
    try {
      const response = await fetch(`${engineUrl}/command`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      });
      const payload = (await response.json()) as CommandResponse;
      if (!response.ok || !payload.ok) {
        throw new Error(payload.message || `HTTP ${response.status}`);
      }

      setLastResponse(payload);
      updateModuleSnapshot(payload, setModules);
      setHistory((current) => [
        { id: Date.now(), requestedAt, request, response: payload },
        ...current,
      ].slice(0, 20));
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : t("terminal.requestFailed");
      setError(message);
      setHistory((current) => [
        { id: Date.now(), requestedAt, request, error: message },
        ...current,
      ].slice(0, 20));
    } finally {
      setLoading(false);
    }
  };

  return (
    <SidebarLayout title={t("terminal.title")}>
      <section className="panel">
        <h2>{t("terminal.title")}</h2>
        <p className="meta">{t("terminal.subtitle")}</p>
        <form onSubmit={runCommand} style={{ marginTop: 16 }}>
          <div className="grid cols-2">
            <label>
              <span className="meta">{t("terminal.commandLabel")}</span>
              <select
                value={commandType}
                onChange={(event) => setCommandType(event.target.value as CommandType)}
                style={{ display: "block", width: "100%", marginTop: 6, padding: 10 }}
              >
                <option value="ListModules">{t("terminal.commands.listModules")}</option>
                <option value="StartModule">{t("terminal.commands.startModule")}</option>
                <option value="StopModule">{t("terminal.commands.stopModule")}</option>
                <option value="RunExperiment">{t("terminal.commands.runExperiment")}</option>
              </select>
            </label>
            <label>
              <span className="meta">{t("terminal.moduleNameLabel")}</span>
              <input
                value={moduleName}
                onChange={(event) => setModuleName(event.target.value)}
                placeholder={t("terminal.moduleNamePlaceholder")}
                list="terminal-module-catalog"
                disabled={!commandNeedsModuleName(commandType)}
                style={{ marginTop: 6 }}
              />
              <datalist id="terminal-module-catalog">
                {modules.map((module) => (
                  <option key={module.name} value={module.name} />
                ))}
              </datalist>
            </label>
          </div>
          <div className="row" style={{ marginTop: 12 }}>
            <button type="submit" disabled={loading}>
              {loading ? t("terminal.running") : t("terminal.run")}
            </button>
            <span className="meta">{t(`terminal.help.${commandType}`)}</span>
          </div>
        </form>
        {error && <p className="error" role="alert">{error}</p>}
      </section>

      <section className="grid cols-2" style={{ marginTop: 16 }}>
        <article className="panel">
          <h3>{t("terminal.latestResult")}</h3>
          {lastResponse ? (
            <>
              <p>{summarizeCommandResponse(lastResponse)}</p>
              {lastResponse.nextPath && (
                <Link className="button-link" href={lastResponse.nextPath}>
                  {t("terminal.openWorkspace")}
                </Link>
              )}
              <pre>{JSON.stringify(lastResponse, null, 2)}</pre>
            </>
          ) : (
            <p className="meta">{t("terminal.noResult")}</p>
          )}
        </article>

        <article className="panel">
          <h3>{t("terminal.moduleSnapshot")}</h3>
          {modules.length > 0 ? (
            <table style={{ width: "100%" }}>
              <thead>
                <tr>
                  <th>{t("terminal.module")}</th>
                  <th>{t("terminal.version")}</th>
                  <th>{t("terminal.status")}</th>
                  <th>{t("terminal.capabilities")}</th>
                </tr>
              </thead>
              <tbody>
                {modules.map((module) => (
                  <tr key={module.name}>
                    <td>
                      <code>{module.name}</code>
                      {module.description && <div className="meta">{module.description}</div>}
                    </td>
                    <td>{module.version ?? "—"}</td>
                    <td>{module.status}</td>
                    <td>{module.capabilities?.join(", ") || "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="meta">{t("terminal.noModules")}</p>
          )}
        </article>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <h3>{t("terminal.history")}</h3>
          <button type="button" onClick={() => setHistory([])} disabled={history.length === 0}>
            {t("terminal.clearHistory")}
          </button>
        </div>
        {history.length > 0 ? history.map((entry) => (
          <article key={entry.id} className="panel" style={{ marginTop: 10, background: "#0b1425" }}>
            <p className="meta">
              {new Date(entry.requestedAt).toLocaleTimeString()} · {entry.request.commandType}
            </p>
            <pre>{JSON.stringify(entry.response ?? { ok: false, message: entry.error }, null, 2)}</pre>
          </article>
        )) : <p className="meta">{t("terminal.noHistory")}</p>}
      </section>
    </SidebarLayout>
  );
}

function updateModuleSnapshot(
  response: CommandResponse,
  setModules: (updater: (current: ModuleInfo[]) => ModuleInfo[]) => void,
) {
  if (response.modules) {
    setModules(() => response.modules ?? []);
    return;
  }
  if (!response.module) return;
  setModules((current) => {
    const next = current.filter((module) => module.name !== response.module?.name);
    next.push(response.module as ModuleInfo);
    return next.sort((left, right) => left.name.localeCompare(right.name));
  });
}
