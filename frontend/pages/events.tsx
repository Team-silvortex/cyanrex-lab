import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { useConfirmedAction } from "../src/components/useConfirmedAction";
import { buildEventDeleteParams } from "../src/features/events/deleteScope";
import { getEngineUrl } from "../src/config/runtime";
import type { EngineEvent } from "../src/features/events/eventStream";
import { useEventHistory } from "../src/features/events/useEventHistory";
import { useEventActions } from "../src/features/events/useEventActions";
import { useEventReadAcknowledgement } from "../src/features/events/useEventReadAcknowledgement";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

type SafetyTone = "ok" | "warn";

export default function EventsPage() {
  const { t } = useI18n();
  const router = useRouter();
  const safety = useConfirmedAction();
  const [streamRevision, setStreamRevision] = useState(0);
  const [scopeError, setScopeError] = useState({ key: "", message: "" });
  const [categoryFilter, setCategoryFilter] = useState<"all" | "kernel" | "platform">(
    () => loadPageState<"all" | "kernel" | "platform">("events_category_v1") ?? "all",
  );
  const [severityFilter, setSeverityFilter] = useState<"all" | "success" | "warning" | "error">(
    () => loadPageState<"all" | "success" | "warning" | "error">("events_severity_v1") ?? "all",
  );
  const [rangePreset, setRangePreset] = useState<"all" | "10m" | "1h" | "24h" | "custom">(
    () => loadPageState<"all" | "10m" | "1h" | "24h" | "custom">("events_range_v1") ?? "all",
  );
  const [startTime, setStartTime] = useState(() => loadPageState<string>("events_start_v1") ?? "");
  const [endTime, setEndTime] = useState(() => loadPageState<string>("events_end_v1") ?? "");
  const [exportFormat, setExportFormat] = useState<"json" | "csv">(
    () => loadPageState<"json" | "csv">("events_export_v1") ?? "json",
  );
  const engineUrl = useMemo(getEngineUrl, []);
  const filters = useMemo(() => ({ categoryFilter, severityFilter, rangePreset, startTime, endTime }),
    [categoryFilter, severityFilter, rangePreset, startTime, endTime]);
  const filterKey = JSON.stringify([router.asPath, filters]);
  const refreshHistory = useCallback(() => setStreamRevision(value => value + 1), []);
  const read = useEventReadAcknowledgement(engineUrl, filterKey);
  const { events, connection, streamGap, valid } = useEventHistory(engineUrl, router.asPath, filters, streamRevision, read.scheduleMarkRead);
  const actions = useEventActions(engineUrl, router.asPath, filters, exportFormat, refreshHistory, t);
  const error = !valid ? t("safety.invalidRange") : actions.error || (scopeError.key === filterKey ? scopeError.message : null);

  const activeFilterCount = useMemo(() => {
    let count = 0;
    if (categoryFilter !== "all") count += 1;
    if (severityFilter !== "all") count += 1;
    if (rangePreset !== "all") count += 1;
    if (rangePreset === "custom" && typeof startTime === "string" && startTime.trim()) count += 1;
    if (rangePreset === "custom" && typeof endTime === "string" && endTime.trim()) count += 1;
    return count;
  }, [categoryFilter, severityFilter, rangePreset, startTime, endTime]);

  useEffect(() => {
    savePageState("events_category_v1", categoryFilter);
    savePageState("events_severity_v1", severityFilter);
    savePageState("events_range_v1", rangePreset);
    savePageState("events_start_v1", startTime);
    savePageState("events_end_v1", endTime);
    savePageState("events_export_v1", exportFormat);
  }, [categoryFilter, severityFilter, rangePreset, startTime, endTime, exportFormat]);

  const deleteFilteredEvents = () => {
    if (!valid || connection !== "open" || events.length === 0) return;
    let params: URLSearchParams;
    try { params = buildEventDeleteParams(filters); }
    catch { setScopeError({ key: filterKey, message: t("safety.invalidRange") }); return; }
    setScopeError({ key: filterKey, message: "" });
    safety.request({ action: t("events.deleteFiltered"), description: t("safety.deleteEvents"), phrase: "DELETE", details: [
      { label: t("events.category"), value: t(`events.${categoryFilter}`) },
      { label: t("events.severity"), value: t(`events.${severityFilter}`) },
      { label: t("events.start"), value: params.get("start") || t("events.all") },
      { label: t("events.end"), value: params.get("end")! },
    ] }, signal => actions.performDelete(params, signal));
  };

  return (
    <SidebarLayout title={t("events.title")}>
      {safety.dialog}
      <div className="events-workspace">
        <section className="panel events-controls" data-testid="events-controls" aria-label={t("events.title")}>
          <div className="section-heading">
            <div>
              <h2>{t("events.title")}</h2>
              <div className="meta events-summary">
                <span>{t("events.status")}: {t(`events.connection${connection[0].toUpperCase()}${connection.slice(1)}`)}</span>
                <span>{t("events.total")}: {events.length}</span>
                <span>{t("events.filtered")}: {events.length}</span>
                <span>{t("events.activeFilters", { count: activeFilterCount })}</span>
              </div>
            </div>
            {connection === "closed" && valid && (
              <button type="button" className="button-secondary" onClick={() => setStreamRevision((revision) => revision + 1)}>{t("events.retryStream")}</button>
            )}
          </div>
          <div className="events-filter-grid" data-testid="events-filters">
            <label className="field-label">
              {t("events.category")}
              <select aria-label={t("events.category")} value={categoryFilter} onChange={(event) => setCategoryFilter(event.target.value as typeof categoryFilter)}>
                <option value="all">{t("events.all")}</option>
                <option value="kernel">{t("events.kernel")}</option>
                <option value="platform">{t("events.platform")}</option>
              </select>
            </label>
            <label className="field-label">
              {t("events.severity")}
              <select aria-label={t("events.severity")} value={severityFilter} onChange={(event) => setSeverityFilter(event.target.value as typeof severityFilter)}>
                <option value="all">{t("events.all")}</option>
                <option value="success">{t("events.success")}</option>
                <option value="warning">{t("events.warning")}</option>
                <option value="error">{t("events.error")}</option>
              </select>
            </label>
            <label className="field-label events-range-field">
              {t("events.range")}
              <select aria-label={t("events.range")} value={rangePreset} onChange={(event) => setRangePreset(event.target.value as typeof rangePreset)}>
                <option value="all">{t("events.all")}</option>
                <option value="10m">{t("events.last10m")}</option>
                <option value="1h">{t("events.last1h")}</option>
                <option value="24h">{t("events.last24h")}</option>
                <option value="custom">{t("events.custom")}</option>
              </select>
            </label>
            {rangePreset === "custom" && (
              <>
                <label className="field-label events-time-field">
                  {t("events.start")}
                  <input type="datetime-local" aria-label={t("events.start")} value={startTime} onChange={(event) => setStartTime(event.target.value)} />
                </label>
                <label className="field-label events-time-field">
                  {t("events.end")}
                  <input type="datetime-local" aria-label={t("events.end")} value={endTime} onChange={(event) => setEndTime(event.target.value)} />
                </label>
              </>
            )}
          </div>
          <div className="events-actions" data-testid="events-actions">
            <label className="field-label events-export-field">
              {t("events.export")}
              <select aria-label={t("events.export")} value={exportFormat} onChange={(event) => setExportFormat(event.target.value as typeof exportFormat)}>
                <option value="json">{t("events.exportJson")}</option>
                <option value="csv">{t("events.exportCsv")}</option>
              </select>
            </label>
            <div className="row events-safe-buttons">
              <button type="button" className="button-secondary" onClick={refreshHistory} disabled={!valid || safety.busy}>{t("events.refreshHistory")}</button>
              <button type="button" className="button-primary" onClick={actions.exportEvents} disabled={!valid || actions.exporting}>{t("events.exportDownload")}</button>
              {actions.exporting && <span className="meta" role="status">{t("events.exporting")}</span>}
            </div>
          </div>
          <div className="events-danger-actions" data-testid="events-danger-actions" role="group" aria-label={t("events.deleteFiltered")}>
            <button type="button" className="button-danger" disabled={!valid || connection !== "open" || safety.busy || events.length === 0}
              onClick={deleteFilteredEvents}>{t("events.deleteFiltered")}</button>
          </div>
          {error && <p className="error" role="alert">{error}</p>}
          <div className="events-read-status">
            {streamGap && <p className="meta" role="status">{t("events.streamGap")}</p>}
            <p className="meta">{t("events.markReadScope")}</p>
            {read.readState === "pending" && <p className="meta" role="status">{t("events.markReadPending")}</p>}
            {read.readState === "error" && <div className="error" role="alert">
              <p>{t("events.markReadFailed")}</p>
              <button type="button" className="button-secondary" onClick={read.retryMarkRead}>{t("events.retryMarkRead")}</button>
            </div>}
          </div>
        </section>

        <section className="panel events-list" data-testid="events-list">
          {valid && events.length === 0 && <p className="meta">{t(connection === "open" ? "events.noEvents" : "events.loadingHistory")}</p>}
          {events.map((_, reverseIdx) => {
            const idx = events.length - 1 - reverseIdx;
            const event = events[idx];
            if (!event) return null;

            const safetyBadges = extractSafetyBadges(event, t);
            return (
              <article key={`${event.timestamp}-${reverseIdx}`} className="panel event-card">
                <p style={{ margin: 0 }}>
                  <strong>{event.event_type}</strong>
                </p>
                <p className="meta event-metadata">
                  <span>{new Date(event.timestamp).toLocaleString()}</span>
                  <span>{t("events.sourceField")}: {event.source}</span>
                  <span>{t("events.categoryField")}: {event.category}</span>
                </p>
                <p className={`event-tag ${event.color}`} style={{ margin: "0 0 8px 0" }}>
                  {event.severity.toUpperCase()}
                </p>
                {safetyBadges.length > 0 && (
                  <div className="row" style={{ marginBottom: 8 }}>
                    {safetyBadges.map((badge, badgeIdx) => (
                      <span key={`${event.timestamp}-${idx}-safety-${badgeIdx}`} className={`safety-tag ${badge.tone}`}>
                        {badge.text}
                      </span>
                    ))}
                  </div>
                )}
                <pre className="event-payload">{JSON.stringify(event.payload, null, 2)}</pre>
              </article>
            );
          })}
        </section>
      </div>
    </SidebarLayout>
  );
}

function extractSafetyBadges(
  event: EngineEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
): Array<{ text: string; tone: SafetyTone }> {
  if (event.event_type !== "ebpf.detached") return [];

  const badges: Array<{ text: string; tone: SafetyTone }> = [];
  const clean = typeof event.payload.clean === "boolean" ? event.payload.clean : undefined;
  if (clean === true) {
    badges.push({ text: t("events.detachClean"), tone: "ok" });
  } else if (clean === false) {
    badges.push({ text: t("events.detachWithRisk"), tone: "warn" });
  }

  const notes = Array.isArray(event.payload.safety_notes)
    ? event.payload.safety_notes.filter((item): item is string => typeof item === "string")
    : [];

  for (const note of notes) {
    badges.push({
      text: mapSafetyNoteToLabel(note, t),
      tone: "warn",
    });
  }

  return badges;
}

function mapSafetyNoteToLabel(
  note: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  if (note.includes("still exists after detach")) return t("events.residualPinPath");
  if (note.includes("still tracked in attachment set")) return t("events.attachmentTrackingResidue");
  if (note.includes("detach all requested but")) return t("events.detachAllIncomplete");
  return note;
}
