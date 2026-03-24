import dynamic from "next/dynamic";
import { loader } from "@monaco-editor/react";
import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { analyzeCCode } from "../src/utils/cAnalyzer";
import { registerEbpfIntelligence } from "../src/utils/cEbpfIntelligence";
import { loadPageState, savePageState } from "../src/utils/pageState";
import { sanitizeForDisplay } from "../src/utils/security";

const MonacoEditor = dynamic(() => import("@monaco-editor/react"), {
  ssr: false,
});

type EbpfRunResponse = {
  success: boolean;
  stage: string;
  message: string;
  compile_stdout: string;
  compile_stderr: string;
  load_stdout: string;
  load_stderr: string;
  pin_path?: string | null;
};

type EbpfRuntimeBackend = "bpftool" | "aya";

type EbpfDetachResponse = {
  ok: boolean;
  message: string;
  detached: string[];
  clean?: boolean;
  safety_notes?: string[];
};

type EbpfAttachmentDetail = {
  pin_path: string;
  source: string;
  program_name: string;
};

type EbpfAttachmentDetailListResponse = {
  attachments: EbpfAttachmentDetail[];
};

type EbpfTemplate = {
  id: string;
  name: string;
  description: string;
  capability: string;
  code: string;
};

type UserScript = {
  id: string;
  username: string;
  title: string;
  script: string;
  created_at: string;
  updated_at: string;
};

type SelectedHeaderMetadata = {
  id: string;
  include_hint: string;
  local_path: string;
};

type HeaderSelectionMetadata = {
  selected_headers: SelectedHeaderMetadata[];
};

const SAMPLE_EBPF = `#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";`;

const MAX_UPLOAD_BYTES = 256 * 1024;

export default function EbpfPage() {
  const { t } = useI18n();
  const [code, setCode] = useState(() => loadPageState<string>("ebpf_code_v1") ?? SAMPLE_EBPF);
  const [result, setResult] = useState<EbpfRunResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [injectedMetadata, setInjectedMetadata] = useState<SelectedHeaderMetadata[]>([]);
  const [attachmentDetails, setAttachmentDetails] = useState<EbpfAttachmentDetail[]>([]);
  const [templates, setTemplates] = useState<EbpfTemplate[]>([]);
  const [selectedTemplate, setSelectedTemplate] = useState(
    () => loadPageState<string>("ebpf_selected_template_v1") ?? "",
  );
  const [scriptTitle, setScriptTitle] = useState(
    () => loadPageState<string>("ebpf_script_title_v1") ?? "untitled-ebpf",
  );
  const [savedScripts, setSavedScripts] = useState<UserScript[]>([]);
  const [samplingPerSec, setSamplingPerSec] = useState(
    () => loadPageState<number>("ebpf_sampling_v1") ?? 20,
  );
  const [streamSeconds, setStreamSeconds] = useState(
    () => loadPageState<number>("ebpf_stream_seconds_v1") ?? 10,
  );
  const [enableKernelStream, setEnableKernelStream] = useState(
    () => loadPageState<boolean>("ebpf_kernel_stream_v1") ?? true,
  );
  const [runtimeBackend, setRuntimeBackend] = useState<EbpfRuntimeBackend>(
    () => (loadPageState<EbpfRuntimeBackend>("ebpf_runtime_backend_v1") ?? "bpftool"),
  );
  const monacoRef = useRef<any>(null);
  const intelligenceRef = useRef<{ dispose: () => void } | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const injectedIncludes = useMemo(
    () => injectedMetadata.map((item) => toIncludePath(item.include_hint)),
    [injectedMetadata],
  );
  const attachments = useMemo(
    () => attachmentDetails.map((item) => item.pin_path),
    [attachmentDetails],
  );

  const analysis = useMemo(
    () => analyzeCCode(code, injectedIncludes),
    [code, injectedIncludes],
  );

  useEffect(() => {
    loader.config({
      paths: {
        vs: "/monaco/vs",
      },
    });

    return () => {
      intelligenceRef.current?.dispose();
      intelligenceRef.current = null;
    };
  }, []);

  useEffect(() => {
    savePageState("ebpf_code_v1", code);
    savePageState("ebpf_selected_template_v1", selectedTemplate);
    savePageState("ebpf_script_title_v1", scriptTitle);
    savePageState("ebpf_sampling_v1", samplingPerSec);
    savePageState("ebpf_stream_seconds_v1", streamSeconds);
    savePageState("ebpf_kernel_stream_v1", enableKernelStream);
    savePageState("ebpf_runtime_backend_v1", runtimeBackend);
  }, [
    code,
    selectedTemplate,
    scriptTitle,
    samplingPerSec,
    streamSeconds,
    enableKernelStream,
    runtimeBackend,
  ]);

  const refreshInjectedMetadata = async () => {
    try {
      const response = await fetch(`${engineUrl}/modules/c-headers/selected-metadata`, {
        credentials: "include",
      });
      if (!response.ok) return;

      const json = (await response.json()) as HeaderSelectionMetadata;
      setInjectedMetadata(json.selected_headers ?? []);
    } catch {
      // ignore metadata refresh errors for now
    }
  };

  useEffect(() => {
    refreshInjectedMetadata();
  }, []);

  const refreshAttachments = async () => {
    try {
      const response = await fetch(`${engineUrl}/ebpf/attachments/details`, {
        credentials: "include",
      });
      if (!response.ok) return;
      const json = (await response.json()) as EbpfAttachmentDetailListResponse;
      setAttachmentDetails(json.attachments ?? []);
    } catch {
      // ignore attachment refresh errors
    }
  };

  useEffect(() => {
    refreshAttachments();
  }, []);

  const refreshScripts = async () => {
    try {
      const response = await fetch(`${engineUrl}/scripts`, {
        credentials: "include",
      });
      if (!response.ok) return;
      const json = (await response.json()) as UserScript[];
      setSavedScripts(json ?? []);
    } catch {
      // ignore script list refresh errors
    }
  };

  useEffect(() => {
    refreshScripts();
  }, [engineUrl]);

  useEffect(() => {
    const loadTemplates = async () => {
      try {
        const response = await fetch(`${engineUrl}/ebpf/templates`, {
          credentials: "include",
        });
        if (!response.ok) return;
        const json = (await response.json()) as EbpfTemplate[];
        setTemplates(json);
      } catch {
        // ignore template fetch errors for now
      }
    };

    loadTemplates();
  }, [engineUrl]);

  useEffect(() => {
    if (!monacoRef.current) return;
    const model = monacoRef.current.editor.getModels()[0];
    if (!model) return;

    applyMarkers(
      { getModel: () => model },
      monacoRef.current,
      analysis.diagnostics,
    );
  }, [analysis.diagnostics]);

  const onUpload = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    if (file.size > MAX_UPLOAD_BYTES) {
      setError(`Upload blocked: file is larger than ${MAX_UPLOAD_BYTES} bytes.`);
      return;
    }

    const text = await file.text();
    setCode(text);
    setError(null);
  };

  const runEbpf = async () => {
    if (code.length > MAX_UPLOAD_BYTES) {
      setError(`Upload blocked: code is larger than ${MAX_UPLOAD_BYTES} bytes.`);
      return;
    }

    const selectedTemplateDef = templates.find((item) => item.id === selectedTemplate);
    const resolvedProgramName = selectedTemplateDef?.name || scriptTitle.trim() || "custom";

    setRunning(true);
    setError(null);
    setResult(null);

    try {
      const response = await fetch(`${engineUrl}/ebpf/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          code,
          template_id: selectedTemplate || null,
          program_name: resolvedProgramName,
          sampling_per_sec: samplingPerSec,
          stream_seconds: streamSeconds,
          enable_kernel_stream: enableKernelStream,
          runtime_backend: runtimeBackend,
        }),
      });

      const json = (await response.json()) as EbpfRunResponse;
      setResult(json);
      await refreshAttachments();

      if (!response.ok) {
        setError(`HTTP ${response.status}: ${json.message}`);
      }
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setRunning(false);
    }
  };

  const saveCurrentScript = async () => {
    setError(null);
    try {
      const response = await fetch(`${engineUrl}/scripts/save`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          title: scriptTitle.trim() || "untitled-ebpf",
          script: code,
        }),
      });
      const json = (await response.json()) as { ok: boolean; message: string };
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }
      await refreshScripts();
    } catch (err) {
      setError((err as Error).message);
    }
  };

  const deleteScript = async (id: string) => {
    setError(null);
    try {
      const response = await fetch(`${engineUrl}/scripts/delete`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ id }),
      });
      const json = (await response.json()) as { ok: boolean; message: string };
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }
      await refreshScripts();
    } catch (err) {
      setError((err as Error).message);
    }
  };

  const detach = async (pinPath?: string) => {
    setError(null);
    try {
      const response = await fetch(`${engineUrl}/ebpf/detach`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ pin_path: pinPath ?? null }),
      });
      const json = (await response.json()) as EbpfDetachResponse;
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }
      if (json.clean === false && (json.safety_notes?.length ?? 0) > 0) {
        setError(`Detach warning: ${json.safety_notes?.join(" | ")}`);
      }
      setResult((prev) =>
        prev
          ? {
              ...prev,
              message: `${prev.message} | detached: ${json.detached.length} | clean: ${
                json.clean === false ? "no" : "yes"
              }`,
            }
          : prev,
      );
      await refreshAttachments();
    } catch (err) {
      setError((err as Error).message);
    }
  };

  const onEditorMount = (editor: any, monaco: any) => {
    monacoRef.current = monaco;

    monaco.editor.defineTheme("cyanrex-c", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword", foreground: "7aa2ff" },
        { token: "string", foreground: "9cd67a" },
        { token: "comment", foreground: "6f86b7" },
      ],
      colors: {
        "editor.background": "#0b1425",
        "editorLineNumber.foreground": "#5d7bb1",
        "editorCursor.foreground": "#9ec0ff",
      },
    });

    monaco.editor.setTheme("cyanrex-c");
    if (!intelligenceRef.current) {
      intelligenceRef.current = registerEbpfIntelligence(monaco);
    }
    applyMarkers(editor, monaco, analysis.diagnostics);
  };

  const onEditorChange = (value: string | undefined) => {
    const next = value ?? "";
    setCode(next);

    if (monacoRef.current) {
      const model = monacoRef.current.editor.getModels()[0];
      if (model) {
        applyMarkers(
          { getModel: () => model },
          monacoRef.current,
          analyzeCCode(next, injectedIncludes).diagnostics,
        );
      }
    }
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

        <div className="grid cols-2" style={{ marginTop: 12 }}>
          <div>
            <p className="meta" style={{ marginTop: 0 }}>Template</p>
            <select
              value={selectedTemplate}
              onChange={(event) => {
                const nextId = event.target.value;
                setSelectedTemplate(nextId);
                const template = templates.find((item) => item.id === nextId);
                if (template) setCode(template.code);
              }}
              style={{ width: "100%", padding: 10, borderRadius: 10 }}
            >
              <option value="">{t("ebpf.selectTemplate")}</option>
              {templates.map((template) => (
                <option key={template.id} value={template.id}>
                  {template.name} ({template.capability})
                </option>
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
                  onChange={(event) => setRuntimeBackend(event.target.value as EbpfRuntimeBackend)}
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
        <div className="row" style={{ marginBottom: 8 }}>
          <button type="button" onClick={refreshInjectedMetadata}>{t("ebpf.refreshInjectedHeaders")}</button>
        </div>

        <p className="meta">{t("ebpf.codeSize")}: {analysis.metadata.lines} lines | {analysis.metadata.bytes} bytes</p>
        <p className="meta">{t("ebpf.includes")}: {analysis.metadata.includes.join(", ") || "(none)"}</p>
        <p className="meta">{t("ebpf.injectedIncludes")}: {analysis.metadata.injectedIncludes.join(", ") || "(none)"}</p>
        <p className="meta">
          {t("ebpf.hookSections")}: {analysis.metadata.sections.map((s) => `${s.name}@L${s.line}`).join(", ") || "(none)"}
        </p>
        <p className="meta">{t("ebpf.hookSectionsMeaning")}</p>
        <p className="meta">
          {t("ebpf.cFunctions")}: {analysis.metadata.functions.map((f) => `${f.name}@L${f.line}`).join(", ") || "(none)"}
        </p>
        <p className="meta">{t("ebpf.cFunctionsMeaning")}</p>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.injectedHeaders")}</h3>
        {injectedMetadata.length === 0 && <p className="meta">{t("ebpf.noInjectedMetadata")}</p>}
        {injectedMetadata.map((item) => (
          <p key={item.id} className="meta">
            {item.include_hint} - {item.local_path}
          </p>
        ))}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("ebpf.diagnostics")}</h3>
        {analysis.diagnostics.length === 0 && <p className="meta">{t("ebpf.noDiagnostics")}</p>}
        {analysis.diagnostics.map((d, idx) => (
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
              <span className="event-tag green">{item.program_name || "custom"}</span>
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

      <section className="panel" style={{ marginTop: 16 }}>
        <h3>{t("ebpf.result")}</h3>
        {!result && !error && <p className="meta">{t("ebpf.noRunResult")}</p>}
        {error && <p className="error">{sanitizeForDisplay(error)}</p>}
        {result && (
          <>
            <p><strong>success:</strong> {String(result.success)}</p>
            <p><strong>stage:</strong> {sanitizeForDisplay(result.stage)}</p>
            <p><strong>message:</strong> {sanitizeForDisplay(result.message)}</p>
            <p><strong>pin_path:</strong> {sanitizeForDisplay(result.pin_path || "(none)")}</p>

            <h4>{t("ebpf.compileStdout")}</h4>
            <pre>{sanitizeForDisplay(result.compile_stdout || "(empty)")}</pre>

            <h4>{t("ebpf.compileStderr")}</h4>
            <pre>{sanitizeForDisplay(result.compile_stderr || "(empty)")}</pre>

            <h4>{t("ebpf.loadStdout")}</h4>
            <pre>{sanitizeForDisplay(result.load_stdout || "(empty)")}</pre>

            <h4>{t("ebpf.loadStderr")}</h4>
            <pre>{sanitizeForDisplay(result.load_stderr || "(empty)")}</pre>
          </>
        )}
      </section>
    </SidebarLayout>
  );
}

function applyMarkers(editor: any, monaco: any, diagnostics: ReturnType<typeof analyzeCCode>["diagnostics"]) {
  const model = editor.getModel?.();
  if (!model) return;

  const markers = diagnostics.map((d) => ({
    startLineNumber: d.line,
    startColumn: d.column,
    endLineNumber: d.line,
    endColumn: d.endColumn,
    message: d.message,
    severity:
      d.severity === "error"
        ? monaco.MarkerSeverity.Error
        : d.severity === "warning"
          ? monaco.MarkerSeverity.Warning
          : monaco.MarkerSeverity.Info,
  }));

  monaco.editor.setModelMarkers(model, "cyanrex-c-analyzer", markers);
}

function toIncludePath(includeHint: string): string {
  const match = includeHint.match(/[<"]([^>"]+)[>"]/);
  return match ? match[1] : includeHint;
}
