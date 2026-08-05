import { useMemo } from "react";
import dynamic from "next/dynamic";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { sanitizeForDisplay } from "../src/utils/security";
import EbpfResultPanel from "../src/features/ebpf/EbpfResultPanel";
import { useEbpfPageController } from "../src/features/ebpf/useEbpfPageController";

const MonacoEditor = dynamic(() => import("@monaco-editor/react"), {
  ssr: false,
});

export default function EbpfPage() {
  const { t } = useI18n();
  const {
    analysis,
    attachments,
    injectedMetadata,
    attachmentDetails,
    code,
    compiler,
    headerInjectionCheck,
    runHeaderInjectionSelfCheck,
    deleteScript,
    detach,
    diagnostics,
    enableKernelStream,
    error,
    onEditorChange,
    onEditorMount,
    onUpload,
    result,
    runEbpf,
    refreshInjectedMetadata,
    runtimeBackend,
    saveCurrentScript,
    debugBreakpoints,
    clearDebugBreakpoints,
    samplingPerSec,
    scriptTitle,
    savedScripts,
    selectedTemplate,
    setCode,
    setEnableKernelStream,
    setRuntimeBackend,
    setScriptTitle,
    setSamplingPerSec,
    setSelectedTemplate,
    running,
    setStreamSeconds,
    streamSeconds,
    templates,
  } = useEbpfPageController(t);

  const onTemplateChange = (nextId: string) => {
    setSelectedTemplate(nextId);
    const template = templates.find((item) => item.id === nextId);
    if (template) {
      setCode(template.code);
    }
  };

  const categorizedTemplates = useMemo(() => {
    const groups: Record<string, typeof templates> = {};
    for (const template of templates) {
      const key = (template.category || template.capability || "other").trim().toLowerCase();
      if (!groups[key]) {
        groups[key] = [];
      }
      groups[key].push(template);
    }
    for (const key of Object.keys(groups)) {
      groups[key].sort((a, b) => a.name.localeCompare(b.name, "en"));
    }
    return groups;
  }, [templates]);

  const categoryOrder = useMemo(() => {
    const keys = Object.keys(categorizedTemplates);

    const topPriority = [
      "learning",
      "learning-plus",
      "xdp",
      "tracepoint",
      "kprobe",
      "kretprobe",
      "ringbuf",
      "other",
    ];
    const levelPriority: Record<string, string[]> = {
      learning: ["foundations", "intermediate", "advanced", "practice", "lab", "core"],
      "learning-plus": ["cases", "track", "advanced", "practice", "beginner", "intermediate", "lab", "core"],
      "learning/foundations/beginner": ["fundamentals", "protocols", "forensics", "operators"],
      "learning/foundations/intermediate": ["protocols", "fundamentals", "forensics", "operators"],
      "learning-plus/cases/advanced": ["forensics", "fundamentals", "protocols", "operators"],
      "learning-plus/track/practice": ["operators", "fundamentals", "protocols", "forensics"],
      "learning/foundations": ["beginner", "intermediate", "advanced", "practice", "lab", "core"],
      "learning-plus/cases": ["advanced", "intermediate", "beginner", "practice", "lab", "core"],
      "learning-plus/track": ["practice", "lab", "advanced", "intermediate", "beginner", "core"],
    };
    const leafPriority: Record<string, string[]> = {
      foundations: ["beginner", "intermediate", "advanced", "practice", "lab", "core"],
      cases: ["advanced", "intermediate", "beginner", "practice", "lab", "core"],
      track: ["practice", "lab", "advanced", "intermediate", "beginner", "core"],
      beginner: ["fundamentals", "protocols", "forensics", "operators"],
      intermediate: ["protocols", "fundamentals", "forensics", "operators"],
      advanced: ["forensics", "fundamentals", "protocols", "operators"],
      practice: ["operators", "fundamentals", "protocols", "forensics"],
    };

    const parseCategory = (value: string) => {
      const normalized = value.toLowerCase().trim();
      const parts = normalized.split("/").map((item) => item.trim()).filter(Boolean);
      return parts;
    };

    return keys.sort((a, b) => {
      const leftParts = parseCategory(a);
      const rightParts = parseCategory(b);
      const leftNormalized = leftParts.map((item) => item.trim().toLowerCase());
      const rightNormalized = rightParts.map((item) => item.trim().toLowerCase());
      const maxDepth = Math.max(leftNormalized.length, rightNormalized.length);

      for (let depth = 0; depth < maxDepth; depth += 1) {
        const leftNode = leftNormalized[depth] || "";
        const rightNode = rightNormalized[depth] || "";
        if (!leftNode || !rightNode) {
          if (!leftNode && !rightNode) continue;
          if (!leftNode) return -1;
          return 1;
        }
        if (leftNode === rightNode) {
          continue;
        }

        const leftParent = depth === 0 ? "" : leftNormalized.slice(0, depth).join("/");
        const rightParent = depth === 0 ? "" : rightNormalized.slice(0, depth).join("/");
        const leftPriority = depth === 0 ? topPriority : (levelPriority[leftParent] ?? leafPriority[leftParent.split("/")[depth - 1]] ?? []);
        const rightPriority = depth === 0 ? topPriority : (levelPriority[rightParent] ?? leafPriority[rightParent.split("/")[depth - 1]] ?? []);

        const leftIndex = leftPriority.indexOf(leftNode);
        const rightIndex = rightPriority.indexOf(rightNode);

        if (leftIndex !== -1 || rightIndex !== -1) {
          if (leftIndex === -1) return 1;
          if (rightIndex === -1) return -1;
          if (leftIndex !== rightIndex) return leftIndex - rightIndex;
        }
      }

      return a.localeCompare(b, "en");
    });
  }, [categorizedTemplates]);

  const formatCategoryLabel = (category: string) => {
    const normalized = category.toLowerCase().trim();
    const parts = normalized.split("/").map((item) => item.trim()).filter(Boolean);

    const labels: Record<string, string> = {
      xdp: t("ebpf.templateCategoryXdp"),
      tracepoint: t("ebpf.templateCategoryTracepoint"),
      kprobe: t("ebpf.templateCategoryKprobe"),
      kretprobe: t("ebpf.templateCategoryKretprobe"),
      ringbuf: t("ebpf.templateCategoryRingbuf"),
      learning: t("ebpf.templateCategoryLearning"),
      "learning-plus": t("ebpf.templateCategoryLearningPlus"),
      other: t("ebpf.templateCategoryOther"),
      foundations: t("ebpf.templateCategoryFoundations"),
      cases: t("ebpf.templateCategoryCases"),
      track: t("ebpf.templateCategoryTrack"),
      beginner: t("ebpf.templateCategoryBeginner"),
      intermediate: t("ebpf.templateCategoryIntermediate"),
      advanced: t("ebpf.templateCategoryAdvanced"),
      practice: t("ebpf.templateCategoryPractice"),
      lab: t("ebpf.templateCategoryLab"),
      core: t("ebpf.templateCategoryCore"),
      fundamentals: t("ebpf.templateCategoryFundamentals"),
      protocols: t("ebpf.templateCategoryProtocols"),
      forensics: t("ebpf.templateCategoryForensics"),
      operators: t("ebpf.templateCategoryOperators"),
    };
    if (parts.length === 0) {
      return t("ebpf.templateCategoryOther");
    }

    const toLabel = (part: string) => labels[part] ?? t(`ebpf.templateCategoryUnknown`, { category: part });
    return parts
      .map((part, index) => {
        const label = toLabel(part);
        if (index === 0) {
          return label;
        }
        if (index === 1) {
          return `${t("ebpf.templateCategoryLabelModule")}: ${label}`;
        }
        if (index === 2) {
          return `${t("ebpf.templateCategoryLabelStage")}: ${label}`;
        }
        return `${t("ebpf.templateCategoryLabelTopic")}: ${label}`;
      })
      .join(" / ");
  };

  return (
    <SidebarLayout title={t("ebpf.title")}>
      <section className="panel">
        <h2>{t("ebpf.title")}</h2>
        <p className="meta">{t("ebpf.subtitle")}</p>

        <div className="row" style={{ marginTop: 12 }}>
          <input
            type="text"
            placeholder={t("ebpf.scriptTitle")}
            value={scriptTitle}
            onChange={(event) => setScriptTitle(event.target.value)}
            style={{ maxWidth: 260 }}
          />
          <input type="file" accept=".c,.h,.txt" onChange={onUpload} />
          <button type="button" onClick={saveCurrentScript} disabled={running}>
            {t("ebpf.saveScript")}
          </button>
          <button type="button" onClick={runEbpf} disabled={running}>
            {running ? t("ebpf.running") : t("ebpf.compileRun")}
          </button>
          <button
            type="button"
            onClick={() => detach(result?.pin_path || undefined)}
            disabled={running || (!result?.pin_path && attachments.length === 0)}
          >
            {t("ebpf.detach")}
          </button>
          <button
            type="button"
            onClick={() => detach(undefined)}
            disabled={running || attachments.length === 0}
          >
            {t("ebpf.detachAll")}
          </button>
        </div>

        <div className="row" style={{ marginTop: 12, alignItems: "center", flexWrap: "wrap" }}>
          <p className="meta" style={{ margin: 0 }}>
            {t("ebpf.debugBreakpoints")}: {debugBreakpoints.length > 0 ? debugBreakpoints.join(", ") : t("ebpf.noBreakpoints")}
          </p>
          <button type="button" onClick={clearDebugBreakpoints} disabled={debugBreakpoints.length === 0}>
            {t("ebpf.clearBreakpoints")}
          </button>
          <p className="meta" style={{ margin: 0 }}>
            {t("ebpf.debugBreakpointHint")}
          </p>
        </div>

        <div className="grid cols-2" style={{ marginTop: 12 }}>
          <div>
            <p className="meta" style={{ marginTop: 0 }}>{t("ebpf.templateLabel")}</p>
            <select
              value={selectedTemplate}
              onChange={(event) => onTemplateChange(event.target.value)}
              style={{ width: "100%", padding: 10, borderRadius: 10 }}
            >
              <option value="">{t("ebpf.selectTemplate")}</option>
              {categoryOrder.map((category) => (
                <optgroup
                  key={category}
                  label={formatCategoryLabel(category)}
                >
                  {categorizedTemplates[category]?.map((template) => (
                    <option key={template.id} value={template.id}>
                      {template.name} ({template.capability})
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </div>
          <div>
            <p className="meta" style={{ marginTop: 0 }}>{t("ebpf.kernelStreamControl")}</p>
            <div className="row">
              <label className="meta">
                {t("ebpf.runtimeBackend")}:
                {" "}
                <select
                  value={runtimeBackend}
                  onChange={(event) =>
                    setRuntimeBackend(event.target.value as "bpftool" | "aya")}
                  style={{ marginLeft: 6 }}
                >
                  <option value="bpftool">{t("ebpf.runtimeBpftool")}</option>
                  <option value="aya">{t("ebpf.runtimeAya")}</option>
                </select>
              </label>
              <label className="meta">
                {t("ebpf.samplingPerSec")}:
                {" "}
                <input
                  type="number"
                  min={1}
                  max={200}
                  value={samplingPerSec}
                  onChange={(event) => setSamplingPerSec(Number(event.target.value) || 1)}
                  style={{ width: 90, marginLeft: 6 }}
                />
              </label>
              <label className="meta">
                {t("ebpf.seconds")}:
                {" "}
                <input
                  type="number"
                  min={1}
                  max={120}
                  value={streamSeconds}
                  onChange={(event) => setStreamSeconds(Number(event.target.value) || 1)}
                  style={{ width: 90, marginLeft: 6 }}
                />
              </label>
              <label className="meta" style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <input
                  type="checkbox"
                  checked={enableKernelStream}
                  onChange={(event) => setEnableKernelStream(event.target.checked)}
                />
                {t("ebpf.kernelStream")}
              </label>
            </div>
          </div>
        </div>

        <div className="editor-shell" style={{ marginTop: 12 }}>
          <MonacoEditor
            height="360px"
            language="c"
            value={code}
            onMount={onEditorMount}
            onChange={onEditorChange}
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              lineNumbersMinChars: 3,
              wordWrap: "on",
              smoothScrolling: true,
              automaticLayout: true,
            }}
          />
        </div>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.inlineMetadata")}</h3>
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
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.injectedHeaders")}</h3>
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
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.diagnostics")}</h3>
        {diagnostics.length === 0 && <p className="meta">{t("ebpf.noDiagnostics")}</p>}
        {diagnostics.map((d, idx) => (
          <p key={`${d.line}-${idx}`} className={d.severity === "error" ? "error" : "meta"}>
            [{d.severity.toUpperCase()}] L{d.line}:{d.column} {d.message}
          </p>
        ))}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.attachedPrograms")}</h3>
        {attachments.length === 0 && <p className="meta">{t("ebpf.noAttachedPrograms")}</p>}
        {attachmentDetails.map((item) => (
          <details key={item.pin_path} className="panel" style={{ marginBottom: 10, background: "#0b1425" }}>
            <summary className="row" style={{ cursor: "pointer", listStyle: "none" }}>
              <code style={{ flex: 1 }}>{item.pin_path}</code>
              <span className="event-tag green">{item.program_name || t("ebpf.customProgramName")}</span>
              <button
                type="button"
                onClick={(event) => {
                  event.preventDefault();
                  detach(item.pin_path);
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

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.savedScripts")}</h3>
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
                onClick={() => {
                  setScriptTitle(item.title);
                  setCode(item.script);
                }}
              >
                {t("ebpf.load")}
              </button>
              <button type="button" onClick={() => deleteScript(item.id)}>{t("ebpf.delete")}</button>
            </div>
          </div>
        ))}
      </section>

      <EbpfResultPanel result={result} error={error} t={t} />
    </SidebarLayout>
  );
}
