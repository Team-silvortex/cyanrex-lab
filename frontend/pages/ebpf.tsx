import { useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import Link from "next/link";
import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { sanitizeForDisplay } from "../src/utils/security";
import EbpfResultPanel from "../src/features/ebpf/EbpfResultPanel";
import EbpfTemplateSelector from "../src/features/ebpf/EbpfTemplateSelector";
import CompileBackendSelector from "../src/features/ebpf/CompileBackendSelector";
import { useEbpfPageController } from "../src/features/ebpf/useEbpfPageController";
import { useEbpfSafetyActions } from "../src/features/ebpf/useEbpfSafetyActions";
import { AttemptResumePanel } from "../src/features/learning/AttemptResumePanel";
import { getEngineUrl } from "../src/config/runtime";

const MonacoEditor = dynamic(() => import("@monaco-editor/react"), {
  ssr: false,
});

export default function EbpfPage() {
  const { t } = useI18n();
  const uploadInput = useRef<HTMLInputElement>(null);
  const router = useRouter();
  const hasResume = router.query.attempt !== undefined;
  const resumeKey = JSON.stringify([getEngineUrl(), router.query.lab, router.query.attempt]);
  const [resumeDecision, setResumeDecision] = useState({ key: resumeKey, decided: false });
  if (resumeDecision.key !== resumeKey) setResumeDecision({ key: resumeKey, decided: false });
  const resumePending = hasResume && (resumeDecision.key !== resumeKey || !resumeDecision.decided);
  const activeLabId = useMemo(() => {
    const value = router.query.lab;
    return typeof value === "string" && /^\d{2}-[a-z0-9-]+$/.test(value) ? value : "";
  }, [router.query.lab]);
  const controller = useEbpfPageController(t, activeLabId, !router.isReady || resumePending);
  const safety = useEbpfSafetyActions(controller, t, activeLabId);
  const {
    analysis,
    attachments,
    injectedMetadata,
    attachmentDetails,
    code,
    compiler,
    compileBackends,
    headerInjectionCheck,
    runHeaderInjectionSelfCheck,
    diagnostics,
    enableKernelStream,
    error,
    onEditorChange,
    onEditorMount,
    result,
    refreshInjectedMetadata,
    runtimeBackend,
    saveCurrentScript,
    debugBreakpoints,
    breakpointHits,
    breakpointStreamGap,
    breakpointConnection,
    lastBreakpointHit,
    activeLabProgress,
    applyLearningAttempt,
    clearDebugBreakpoints,
    samplingPerSec,
    scriptTitle,
    savedScripts,
    selectedTemplate,
    setEnableKernelStream,
    setRuntimeBackend,
    setScriptTitle,
    setSamplingPerSec,
    running,
    setStreamSeconds,
    streamSeconds,
    templates,
  } = controller;

  return (
    <SidebarLayout title={t("ebpf.title")}>
      {safety.dialog}
      <div className="ebpf-workspace">
        <header className="page-header">
          <div>
            <p className="brand-kicker">CYANREX WORKSPACE</p>
            <h2>{t("ebpf.title")}</h2>
            <p className="meta">{t("ebpf.subtitle")}</p>
          </div>
        </header>
        {router.isReady && hasResume && <AttemptResumePanel key={resumeKey}
          engineUrl={getEngineUrl()} attemptId={router.query.attempt} labId={router.query.lab} running={running}
          onKeep={() => setResumeDecision({ key: resumeKey, decided: true })} onApply={attempt => {
            if (!applyLearningAttempt(attempt)) return false;
            setResumeDecision({ key: resumeKey, decided: true });
            return true;
          }} />}

        {activeLabProgress && activeLabProgress.lab.id === activeLabId && (
          <div className="learning-context">
            <div>
              <strong>{t("learn.activeLab")}: {activeLabProgress.lab.title}</strong>
              <p className="meta" style={{ margin: "4px 0 0" }}>
                {t(`learn.status.${activeLabProgress.status}`)} · {t("learn.attemptCount", {
                  count: activeLabProgress.attempts,
                })}
              </p>
              {!hasResume && <p className="meta" style={{ margin: "4px 0 0" }}>{t("safety.labDraft")}</p>}
            </div>
            <div className="workspace-links">
              {!hasResume && <button type="button" className="button-secondary" onClick={safety.loadLabTemplate}
                disabled={running || safety.busy || !safety.labTemplateAvailable}>{t("safety.loadLabTemplate")}</button>}
              <Link href={`/learn/${activeLabProgress.lab.doc_slug}`}>{t("learn.openLab")}</Link>
            </div>
          </div>
        )}

        <div className="editor-actionbar">
          <label className="script-title-field">
            <span className="sr-only">{t("ebpf.scriptTitle")}</span>
            <input type="text" placeholder={t("ebpf.scriptTitle")} value={scriptTitle}
              onChange={event => setScriptTitle(event.target.value)} />
          </label>
          <div className="editor-actions">
            <input ref={uploadInput} hidden type="file" accept=".c,.h,.txt" onChange={safety.upload} />
            <button type="button" className="button-secondary" disabled={running || safety.busy} onClick={() => uploadInput.current?.click()}>
              {t("ebpf.importFile")}
            </button>
            <button type="button" className="button-secondary" onClick={saveCurrentScript} disabled={running || safety.busy}>
              {t("ebpf.saveScript")}
            </button>
            <button type="button" className="button-primary" onClick={safety.run}
              disabled={running || safety.busy || !router.isReady || resumePending}>
              {running ? t("ebpf.running") : t("ebpf.compileRun")}
            </button>
          </div>
        </div>

        <div className="workbench-grid">
          <div className="workbench-main">
            <section className="panel source-panel" id="ebpf-source" aria-label={t("ebpf.source")}>
              <div className="source-toolbar">
                <label className="template-field">
                  <span className="meta">{t("ebpf.templateLabel")}</span>
                  <EbpfTemplateSelector templates={templates} selectedTemplate={selectedTemplate}
                    onChange={safety.selectTemplate} disabled={running || safety.busy} t={t} />
                </label>
                <div className="workspace-links">
                  <a href="#runtime-settings">{t("ebpf.runtimeSettings")}</a>
                  <a href="#ebpf-output">{t("ebpf.result")}</a>
                </div>
              </div>
              <div className="editor-shell">
                <MonacoEditor height="100%" language="c" value={code} onMount={onEditorMount} onChange={onEditorChange}
                  options={{ minimap: { enabled: false }, fontSize: 13, lineNumbersMinChars: 3,
                    wordWrap: "on", smoothScrolling: true, automaticLayout: true }} />
              </div>
              <div className="editor-statusbar">
                <span className="meta">C · {analysis.metadata.lines} lines · clang: {compiler.status}</span>
                <span className="meta">{t("ebpf.runtimeBackend")}: {runtimeBackend}</span>
              </div>
              <details className="editor-debug">
                <summary>{t("ebpf.debugBreakpoints")} · {debugBreakpoints.length}
                  {lastBreakpointHit && <span className="event-tag yellow">L{lastBreakpointHit.line} · {breakpointHits.length}</span>}
                </summary>
                <div className="row">
                  <p className="meta">{debugBreakpoints.length > 0 ? debugBreakpoints.join(", ") : t("ebpf.noBreakpoints")}</p>
                  <button type="button" className="button-secondary" onClick={clearDebugBreakpoints} disabled={debugBreakpoints.length === 0}>
                    {t("ebpf.clearBreakpoints")}
                  </button>
                </div>
                <p className="meta">{t("ebpf.debugBreakpointHint")}</p>
                {lastBreakpointHit && <p className="meta">
                  {t("ebpf.debugLastHit")}: L{lastBreakpointHit.line} · {t("ebpf.debugHitCount")}: {breakpointHits.length}
                </p>}
              </details>
              {breakpointStreamGap && <p className="meta" role="status">
                {t("events.streamGap")} ({t(`events.connection${breakpointConnection[0].toUpperCase()}${breakpointConnection.slice(1)}`)})
              </p>}
              <div className="editor-diagnostics">
                <h3>{t("ebpf.diagnostics")} <span className="count-badge">{diagnostics.length}</span></h3>
                {diagnostics.length === 0 && <p className="meta">{t("ebpf.noDiagnostics")}</p>}
                {diagnostics.map((d, idx) => <p key={`${d.line}-${idx}`} className={d.severity === "error" ? "error" : "meta"}>
                  [{d.severity.toUpperCase()}] L{d.line}:{d.column} {d.message}
                </p>)}
              </div>
            </section>
            <div id="ebpf-output" tabIndex={-1}>
              <EbpfResultPanel result={result} error={error} t={t} />
            </div>
          </div>

          <aside className="workbench-aside" aria-label={t("ebpf.runtimeSettings")}>
            <section className="panel runtime-settings" id="runtime-settings" tabIndex={-1}>
              <div className="section-heading">
                <h3>{t("ebpf.runtimeSettings")}</h3>
                <a className="back-to-source" href="#ebpf-source">{t("ebpf.backToSource")}</a>
              </div>
              <label className="field-label">
                <span>{t("ebpf.runtimeBackend")}</span>
                <select aria-label={t("ebpf.runtimeBackend")} value={runtimeBackend}
                  onChange={event => setRuntimeBackend(event.target.value as "bpftool" | "aya")}>
                  <option value="bpftool">{t("ebpf.runtimeBpftool")}</option>
                  <option value="aya">{t("ebpf.runtimeAya")}</option>
                </select>
              </label>
              <CompileBackendSelector {...compileBackends} setTarget={safety.selectCompilerTarget} t={t} />
              <div className="runtime-sampling">
                <label className="field-label">
                  <span>{t("ebpf.samplingPerSec")}</span>
                  <input type="number" min={1} max={200} value={samplingPerSec}
                    onChange={event => setSamplingPerSec(Number(event.target.value) || 1)} />
                </label>
                <label className="field-label">
                  <span>{t("ebpf.seconds")}</span>
                  <input type="number" min={1} max={120} value={streamSeconds}
                    onChange={event => setStreamSeconds(Number(event.target.value) || 1)} />
                </label>
              </div>
              <label className="checkbox-field">
                <input type="checkbox" checked={enableKernelStream} onChange={event => setEnableKernelStream(event.target.checked)} />
                {t("ebpf.kernelStream")}
              </label>
            </section>

            <section className="panel attachments-panel">
              <h3>{t("ebpf.attachedPrograms")} <span className="count-badge">{attachments.length}</span></h3>
              <div className="row">
                <button type="button" className="button-secondary" onClick={() => result?.pin_path && safety.detachOne(result.pin_path)}
                  disabled={running || safety.busy || !result?.pin_path}>{t("ebpf.detach")}</button>
                <button type="button" className="button-danger" onClick={safety.detachAll}
                  disabled={running || safety.busy || attachments.length === 0}>{t("ebpf.detachAll")}</button>
              </div>
              {attachments.length === 0 && <p className="meta">{t("ebpf.noAttachedPrograms")}</p>}
              {attachmentDetails.map((item) => (
                <details key={item.pin_path} className="panel" style={{ marginBottom: 10, background: "#0b1425" }}>
                  <summary className="row" style={{ cursor: "pointer", listStyle: "none" }}>
                    <code style={{ flex: 1 }}>{item.pin_path}</code>
                    <span className="event-tag green">{item.program_name || t("ebpf.customProgramName")}</span>
                    <button
                      type="button"
                      className="button-secondary"
                      disabled={running || safety.busy}
                      onClick={(event) => {
                        event.preventDefault();
                        safety.detachOne(item.pin_path);
                      }}
                    >
                      {t("ebpf.detach")}
                    </button>
                  </summary>
                  <div style={{ marginTop: 10 }}>
                    <p className="meta" style={{ marginTop: 0 }}>{t("ebpf.source")}</p>
                    <pre style={{ margin: 0 }}>{sanitizeForDisplay(item.source || t("ebpf.sourceUnavailable"))}</pre>
                  </div>
                </details>
              ))}
            </section>

            <details className="panel workspace-disclosure">
              <summary>{t("ebpf.savedScripts")} <span className="count-badge">{savedScripts.length}</span></summary>
              {savedScripts.length === 0 && <p className="meta">{t("ebpf.noSavedScripts")}</p>}
              {savedScripts.map((item) => (
                <div key={item.id} className="panel" style={{ marginBottom: 8, background: "#0b1425" }}>
                  <div className="row" style={{ justifyContent: "space-between" }}>
                    <strong>{item.title}</strong>
                    <span className="meta">{new Date(item.updated_at).toLocaleString()}</span>
                  </div>
                  <div className="row" style={{ marginTop: 8 }}>
                    <button
                      type="button"
                      disabled={running || safety.busy}
                      onClick={() => safety.loadScript(item)}
                    >
                      {t("ebpf.load")}
                    </button>
                    <button type="button" className="button-danger" disabled={safety.busy} onClick={() => safety.deleteScript(item)}>{t("ebpf.delete")}</button>
                  </div>
                </div>
              ))}
            </details>
            <details className="panel workspace-disclosure">
              <summary>{t("ebpf.inlineMetadata")}</summary>
              <p className="meta">{t("ebpf.codeSize")}: {analysis.metadata.lines} lines | {analysis.metadata.bytes} bytes | clang: {compiler.status}</p>
              <p className="meta">{t("ebpf.includes")}: {analysis.metadata.includes.join(", ") || t("ebpf.noData")}</p>
              <p className="meta">{t("ebpf.injectedIncludes")}: {analysis.metadata.injectedIncludes.join(", ") || t("ebpf.noData")}</p>
              <p className="meta">
                {t("ebpf.hookSections")}: {analysis.metadata.sections.map((s: { name: string; line: number }) => `${s.name}@L${s.line}`).join(", ") || t("ebpf.noData")}
              </p>
              <p className="meta">{t("ebpf.hookSectionsMeaning")}</p>
              <p className="meta">
                {t("ebpf.cFunctions")}: {analysis.metadata.functions.map((f: { name: string; line: number }) => `${f.name}@L${f.line}`).join(", ") || t("ebpf.noData")}
              </p>
              <p className="meta">{t("ebpf.cFunctionsMeaning")}</p>
            </details>
            <details className="panel workspace-disclosure">
              <summary>{t("ebpf.injectedHeaders")} <span className="count-badge">{injectedMetadata.length}</span></summary>
              <div className="row" style={{ marginBottom: 8 }}>
                <button type="button" onClick={refreshInjectedMetadata}>
                  {t("ebpf.refreshInjectedHeaders")}
                </button>
                <button
                  type="button"
                  onClick={runHeaderInjectionSelfCheck}
                  disabled={headerInjectionCheck.status === "checking"}
                >
                  {t("ebpf.headerInjectionDryRun")}
                </button>
              </div>
              {injectedMetadata.length === 0 && <p className="meta">{t("ebpf.noInjectedMetadata")}</p>}
              {injectedMetadata.map((item) => (
                <p key={item.id} className="meta">
                  <span style={{ fontWeight: 700, marginRight: 8 }}>{item.id}</span>
                  {item.include_hint}
                  {" -> "}
                  {sanitizeForDisplay(item.local_path)}
                  <span
                    className={`event-tag ${item.downloaded ? "green" : "red"}`}
                    style={{ marginLeft: 8 }}
                  >
                    {item.downloaded ? t("modules.downloaded") : t("ebpf.headerMissing")}
                  </span>
                </p>
              ))}
              {headerInjectionCheck.status !== "idle" && (
                <details className="panel" style={{ marginTop: 10, background: "#0b1425" }}>
                  <summary className="row" style={{ cursor: "pointer", listStyle: "none" }}>
                    <span className="meta" style={{ flex: 1 }}>
                      {headerInjectionCheck.status === "checking"
                        ? t("common.checking")
                        : headerInjectionCheck.status === "passed"
                          ? t("ebpf.headerInjectionCheckPassed")
                          : t("ebpf.headerInjectionCheckFailed")}
                    </span>
                    <span
                      className={`event-tag ${
                        headerInjectionCheck.status === "passed"
                          ? "green"
                          : headerInjectionCheck.status === "checking"
                          ? "yellow"
                          : "red"
                      }`}
                    >
                      {headerInjectionCheck.status}
                    </span>
                  </summary>
                  <p className="meta" style={{ marginTop: 8 }}>
                    {sanitizeForDisplay(headerInjectionCheck.message || t("ebpf.noData"))}
                  </p>
                  <p className="meta">
                    {t("ebpf.diagnosticCount")}: {headerInjectionCheck.diagnostics}
                  </p>
                  <p className="meta" style={{ marginBottom: 4 }}>
                    {t("ebpf.compileStdout")}:
                  </p>
                  <pre style={{ margin: "0 0 10px 0" }}>
                    {sanitizeForDisplay(headerInjectionCheck.stdout || t("ebpf.outputEmpty"))}
                  </pre>
                  <p className="meta" style={{ marginBottom: 4 }}>
                    {t("ebpf.compileStderr")}:
                  </p>
                  <pre style={{ margin: 0 }}>
                    {sanitizeForDisplay(headerInjectionCheck.stderr || t("ebpf.outputEmpty"))}
                  </pre>
                </details>
              )}
            </details>
          </aside>
        </div>
      </div>
    </SidebarLayout>
  );
}
