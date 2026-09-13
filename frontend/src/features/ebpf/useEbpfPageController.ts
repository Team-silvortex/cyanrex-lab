import { useEffect, useMemo, useRef, useState } from "react";
import { loader } from "@monaco-editor/react";

import { getEngineUrl } from "../../config/runtime";
import { analyzeCCode } from "../../utils/cAnalyzer";
import { registerEbpfIntelligence } from "../../utils/cEbpfIntelligence";
import { loadPageState, savePageState } from "../../utils/pageState";
import type { LabAttempt, LabProgress } from "../learning/models";
import { MAX_UPLOAD_BYTES, SAMPLE_EBPF } from "./models";
import type { EbpfRuntimeBackend, EbpfTemplate, UserScript } from "./models";
import { applyMarkers, toIncludePath } from "./editorUtils";
import { useBreakpointHitStream } from "./useBreakpointHitStream";
import { useCompilerDiagnostics } from "./useCompilerDiagnostics";
import { useCompileBackends } from "./useCompileBackends";
import { useEbpfEditorBreakpoints } from "./useEditorBreakpoints";
import { useHeaderInjectionCheck } from "./useHeaderInjectionCheck";
import { useSelectedHeaders } from "./useSelectedHeaders";
import { useAttachmentInventory } from "./useAttachmentInventory";
import { useRuntimeActions } from "./useRuntimeActions";

export const buildAyaBackendHint = (
  message: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string | null => {
  const normalized = message.toLowerCase();
  if (normalized.includes("aya runtime backend is supported only on linux")) {
    return t("ebpf.runtimeAyaOnlyOnLinux");
  }
  if (
    normalized.includes("tracepoint attach requires tracefs mount") ||
    normalized.includes("missing tracepoint id path")
  ) {
    return t("ebpf.runtimeAyaTracefsHint");
  }
  if (normalized.includes("aya backend currently supports tracepoint programs only")) {
    return t("ebpf.runtimeAyaTracepointOnlyHint");
  }
  if (normalized.includes("aya failed to attach tracepoint program")) {
    return t("ebpf.runtimeAyaAttachHint");
  }
  if (
    normalized.includes("no tracepoint sec(") ||
    normalized.includes("no tracepoint sec(\"tracepoint")
  ) {
    return t("ebpf.runtimeAyaNoTracepointHint");
  }
  if (normalized.includes("aya requires") || normalized.includes("aya attach")) {
    return t("ebpf.runtimeAyaGeneralHint");
  }
  return null;
};

export function useEbpfPageController(
  t: (key: string, vars?: Record<string, string | number>) => string,
  activeLabId = "",
  runBlocked = false,
  navigationKey = activeLabId,
) {
  const [code, setCode] = useState(() => loadPageState<string>("ebpf_code_v1") ?? SAMPLE_EBPF);
  const [error, setError] = useState<string | null>(null);
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
  const [activeLabProgress, setActiveLabProgress] = useState<LabProgress | null>(null);
  const monacoRef = useRef<any>(null);
  const editorRef = useRef<any>(null);
  const [intelligenceEditor, setIntelligenceEditor] = useState<{ editor: any; monaco: any } | null>(null);
  const activeLabRef = useRef(activeLabId);
  activeLabRef.current = activeLabId;
  const engineUrl = getEngineUrl();
  const { attachmentDetails, attachmentState, refreshAttachments, invalidateAttachments } = useAttachmentInventory(engineUrl);
  const runtimeContext = JSON.stringify([engineUrl, navigationKey, activeLabId, code, selectedTemplate,
    scriptTitle, runtimeBackend, samplingPerSec, streamSeconds, enableKernelStream]);
  const runtime = useRuntimeActions(engineUrl, navigationKey, runtimeContext, invalidateAttachments, refreshAttachments, t);
  const { result, running, detaching, runtimeNotice } = runtime;
  const { injectedMetadata, injectedHeaderContext, refreshInjectedMetadata, injectedMetadataError, injectedMetadataLoading } = useSelectedHeaders(engineUrl);
  const { headerInjectionCheck, runHeaderInjectionSelfCheck } = useHeaderInjectionCheck(code, engineUrl, injectedHeaderContext, t);
  const compileBackends = useCompileBackends(engineUrl);
  const { hits: breakpointHits, streamGap: breakpointStreamGap, connection: breakpointConnection } = useBreakpointHitStream(
    engineUrl,
    result?.debug?.session_id ?? null,
    result?.debug?.instrumented_lines ?? [],
  );
  const lastBreakpointHit = breakpointHits.at(-1) ?? null;
  const { debugBreakpoints, clearDebugBreakpoints, onEditorReadyForDebug } =
    useEbpfEditorBreakpoints({
      code,
      editorRef,
      monacoRef,
      hitLine: lastBreakpointHit?.line,
    });

  const injectedIncludes = useMemo(
    () => injectedMetadata.map((item) => toIncludePath(item.include_hint)),
    [injectedMetadata],
  );
  const attachments = useMemo(
    () => attachmentDetails.map((item) => item.pin_path),
    [attachmentDetails],
  );
  const analysis = useMemo(() => analyzeCCode(code, injectedIncludes), [code, injectedIncludes]);
  const compiler = useCompilerDiagnostics(
    code,
    engineUrl,
    injectedHeaderContext,
    compileBackends.target,
  );
  const diagnostics = useMemo(
    () => [...analysis.diagnostics, ...compiler.diagnostics],
    [analysis.diagnostics, compiler.diagnostics],
  );
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

  const refreshLearningProgress = async () => {
    if (!activeLabId) {
      setActiveLabProgress(null);
      return;
    }
    try {
      const response = await fetch(`${engineUrl}/learning/labs`, { credentials: "include" });
      if (!response.ok) return;
      const progress = (await response.json()) as LabProgress[];
      if (activeLabRef.current === activeLabId) {
        setActiveLabProgress(progress.find((item) => item.lab.id === activeLabId) ?? null);
      }
    } catch {
      // Learning progress is supplementary to the editor runtime.
    }
  };

  useEffect(() => {
    setActiveLabProgress(null);
    void refreshLearningProgress();
  }, [activeLabId, engineUrl]);

  const applyLearningAttempt = (attempt: LabAttempt) => {
    if (runtime.isBusy() || attempt.lab_id !== activeLabId) return false;
    setCode(attempt.source);
    setSelectedTemplate(attempt.template_id ?? "");
    setScriptTitle(`lab-${activeLabId}`);
    runtime.clear();
    setError(null);
    clearDebugBreakpoints();
    return true;
  };

  const refreshScripts = async () => {
    try {
      const response = await fetch(`${engineUrl}/scripts`, { credentials: "include" });
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
        const response = await fetch(`${engineUrl}/ebpf/templates`, { credentials: "include" });
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
    if (!monacoRef.current || !editorRef.current) return;
    applyMarkers(editorRef.current, monacoRef.current, diagnostics);
  }, [diagnostics]);

  useEffect(() => {
    if (!intelligenceEditor) return;
    const registration = registerEbpfIntelligence(intelligenceEditor.monaco, engineUrl, intelligenceEditor.editor, injectedHeaderContext);
    return () => registration.dispose();
  }, [intelligenceEditor, engineUrl, injectedHeaderContext]);

  useEffect(() => {
    loader.config({
      paths: {
        vs: "/monaco/vs",
      },
    });
  }, []);

  const onUpload = async (file: File, signal?: AbortSignal) => {
    if (file.size > MAX_UPLOAD_BYTES) {
      setError(t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }));
      return;
    }
    const text = await file.text();
    if (signal?.aborted) return;
    setCode(text);
    setError(null);
  };

  const runEbpf = async (signal?: AbortSignal) => {
    if (runtime.isBusy() || runBlocked || signal?.aborted) return;
    if (new TextEncoder().encode(code).byteLength > MAX_UPLOAD_BYTES) {
      setError(t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }));
      return;
    }
    if (!code.trim()) { setError(t("ebpf.runEmpty")); return; }

    const selectedTemplateDef = templates.find((item) => item.id === selectedTemplate);
    const resolvedProgramName =
      selectedTemplateDef?.name || scriptTitle.trim() || t("ebpf.customProgramName");

    setError(null);
    const accepted = await runtime.run({
      code, lab_id: activeLabId || null, template_id: selectedTemplate || null, program_name: resolvedProgramName,
      sampling_per_sec: samplingPerSec, stream_seconds: streamSeconds, enable_kernel_stream: enableKernelStream,
      runtime_backend: runtimeBackend, debug_breakpoints: debugBreakpoints,
    }, signal, message => {
      const hint = runtimeBackend === "aya" ? buildAyaBackendHint(message, t) : null;
      return hint ? `${message} ${hint}` : message;
    });
    if (accepted && !signal?.aborted && activeLabRef.current === activeLabId) void refreshLearningProgress();
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
      throw err;
    }
  };

  const detach = async (pinPath: string | null, signal?: AbortSignal) => {
    setError(null);
    await runtime.detach(pinPath, signal);
  };

  const onEditorMount = (editor: any, monaco: any) => {
    monacoRef.current = monaco;
    editorRef.current = editor;

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
    setIntelligenceEditor({ editor, monaco });
    onEditorReadyForDebug(editor, monaco);
    applyMarkers(editor, monaco, analysis.diagnostics);
  };

  const onEditorChange = (value: string | undefined) => {
    const next = value ?? "";
    setCode(next);
    if (monacoRef.current && editorRef.current) {
      applyMarkers(editorRef.current, monacoRef.current, analyzeCCode(next, injectedIncludes).diagnostics);
    }
  };

  return {
    code,
    result,
    error: error || runtime.error,
    running,
    detaching,
    runtimeNotice,
    analysis,
    scriptTitle,
    attachments,
    attachmentDetails,
    attachmentState,
    refreshAttachments,
    compiler,
    compileBackends,
    diagnostics,
    headerInjectionCheck,
    injectedMetadata,
    injectedMetadataError,
    injectedMetadataLoading,
    monacoRef,
    setCode,
    setScriptTitle,
    savedScripts,
    debugBreakpoints,
    breakpointHits,
    breakpointStreamGap,
    breakpointConnection,
    lastBreakpointHit,
    activeLabProgress,
    applyLearningAttempt,
    clearDebugBreakpoints,
    selectedTemplate,
    setSelectedTemplate,
    runtimeBackend,
    samplingPerSec,
    setSamplingPerSec,
    streamSeconds,
    setStreamSeconds,
    enableKernelStream,
    setEnableKernelStream,
    setRuntimeBackend,
    templates,
    onUpload,
    runEbpf,
    saveCurrentScript,
    deleteScript,
    detach,
    onEditorMount,
    onEditorChange,
    refreshInjectedMetadata,
    runHeaderInjectionSelfCheck,
  };
}
