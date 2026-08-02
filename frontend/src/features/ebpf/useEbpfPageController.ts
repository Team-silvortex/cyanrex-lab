import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";
import { loader } from "@monaco-editor/react";

import { getEngineUrl } from "../../config/runtime";
import { analyzeCCode } from "../../utils/cAnalyzer";
import { registerEbpfIntelligence } from "../../utils/cEbpfIntelligence";
import { loadPageState, savePageState } from "../../utils/pageState";
import { MAX_UPLOAD_BYTES, SAMPLE_EBPF } from "./models";
import type {
  EbpfAttachmentDetail,
  EbpfAttachmentDetailListResponse,
  EbpfDetachResponse,
  EbpfCheckResponse,
  EbpfRunResponse,
  EbpfRuntimeBackend,
  EbpfTemplate,
  HeaderSelectionMetadata,
  SelectedHeaderMetadata,
  UserScript,
} from "./models";
import { applyMarkers, toIncludePath } from "./editorUtils";
import { useCompilerDiagnostics } from "./useCompilerDiagnostics";

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

type HeaderInjectionCheckStatus = "idle" | "checking" | "passed" | "issues" | "error";

type HeaderInjectionCheckState = {
  status: HeaderInjectionCheckStatus;
  message: string;
  stdout: string;
  stderr: string;
  diagnostics: number;
};

const INITIAL_HEADER_CHECK_STATE: HeaderInjectionCheckState = {
  status: "idle",
  message: "",
  stdout: "",
  stderr: "",
  diagnostics: 0,
};

export function useEbpfPageController(t: (key: string, vars?: Record<string, string | number>) => string) {
  const [code, setCode] = useState(() => loadPageState<string>("ebpf_code_v1") ?? SAMPLE_EBPF);
  const [result, setResult] = useState<EbpfRunResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [injectedMetadata, setInjectedMetadata] = useState<SelectedHeaderMetadata[]>([]);
  const [headerInjectionCheck, setHeaderInjectionCheck] =
    useState<HeaderInjectionCheckState>(INITIAL_HEADER_CHECK_STATE);
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
  const engineUrl = getEngineUrl();

  const injectedIncludes = useMemo(
    () => injectedMetadata.map((item) => toIncludePath(item.include_hint)),
    [injectedMetadata],
  );
  const attachments = useMemo(
    () => attachmentDetails.map((item) => item.pin_path),
    [attachmentDetails],
  );
  const analysis = useMemo(() => analyzeCCode(code, injectedIncludes), [code, injectedIncludes]);
  const injectedHeaderContext = useMemo(
    () =>
      injectedMetadata
        .map((item) => `${item.id}:${item.include_hint}:${item.local_path}`)
        .sort()
        .join("|"),
    [injectedMetadata],
  );
  const compiler = useCompilerDiagnostics(code, engineUrl, injectedHeaderContext);
  const diagnostics = useMemo(
    () => [...analysis.diagnostics, ...compiler.diagnostics],
    [analysis.diagnostics, compiler.diagnostics],
  );
  useEffect(() => {
    setHeaderInjectionCheck(INITIAL_HEADER_CHECK_STATE);
  }, [code, injectedHeaderContext]);

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

  const runHeaderInjectionSelfCheck = async () => {
    if (!code.trim()) {
      setHeaderInjectionCheck({
        ...INITIAL_HEADER_CHECK_STATE,
        status: "error",
        message: t("ebpf.headerInjectionCheckEmpty"),
      });
      return;
    }

    if (code.length > MAX_UPLOAD_BYTES) {
      setHeaderInjectionCheck({
        ...INITIAL_HEADER_CHECK_STATE,
        status: "error",
        message: t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }),
      });
      return;
    }

    setHeaderInjectionCheck({
      ...INITIAL_HEADER_CHECK_STATE,
      status: "checking",
      message: t("ebpf.headerInjectionCheckRunning"),
    });

    try {
      const response = await fetch(`${engineUrl}/ebpf/check`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ code }),
      });
      const json = (await response.json()) as EbpfCheckResponse;
      const remoteMessage = json.message?.trim();

      setHeaderInjectionCheck({
        status: json.ok && response.ok ? "passed" : "issues",
        message:
          response.ok && json.ok
            ? remoteMessage || t("ebpf.headerInjectionCheckPassed")
            : `${remoteMessage || t("ebpf.headerInjectionCheckFailed")} (HTTP ${response.status})`,
        stdout: json.stdout || "",
        stderr: json.stderr || "",
        diagnostics: json.diagnostics.length,
      });
    } catch (err) {
      setHeaderInjectionCheck({
        ...INITIAL_HEADER_CHECK_STATE,
        status: "error",
        message: (err as Error).message,
      });
    }
  };

  useEffect(() => {
    refreshInjectedMetadata();
  }, [engineUrl]);

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
  }, [engineUrl]);

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
    if (!monacoRef.current) return;
    const model = monacoRef.current.editor.getModels()[0];
    if (!model) return;
    applyMarkers({ getModel: () => model }, monacoRef.current, diagnostics);
  }, [diagnostics]);

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

  const onUpload = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    if (file.size > MAX_UPLOAD_BYTES) {
      setError(t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }));
      return;
    }
    const text = await file.text();
    setCode(text);
    setError(null);
  };

  const runEbpf = async () => {
    if (code.length > MAX_UPLOAD_BYTES) {
      setError(t("ebpf.uploadBlocked", { limit: MAX_UPLOAD_BYTES }));
      return;
    }

    const selectedTemplateDef = templates.find((item) => item.id === selectedTemplate);
    const resolvedProgramName =
      selectedTemplateDef?.name || scriptTitle.trim() || t("ebpf.customProgramName");

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

      const ayaHint = runtimeBackend === "aya" ? buildAyaBackendHint(json.message, t) : null;
      const friendlyError = response.ok ? json.message : `HTTP ${response.status}: ${json.message}`;
      const fullError = ayaHint ? `${friendlyError} ${ayaHint}` : friendlyError;

      if (!response.ok || !json.success) {
        setError(fullError);
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
        setError(t("ebpf.detachWarning", { notes: (json.safety_notes ?? []).join(" | ") }));
      }
      setResult((prev) =>
        prev
          ? {
              ...prev,
              message: `${prev.message} | ${t("ebpf.detachedCount", { count: json.detached.length })} | ${t("ebpf.detachState", {
                state: json.clean === false ? t("ebpf.detachUnclean") : t("ebpf.detachClean"),
              })}`,
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
      intelligenceRef.current = registerEbpfIntelligence(monaco, engineUrl);
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

  return {
    code,
    result,
    error,
    running,
    analysis,
    scriptTitle,
    attachments,
    attachmentDetails,
    compiler,
    diagnostics,
    headerInjectionCheck,
    injectedMetadata,
    monacoRef,
    setCode,
    setScriptTitle,
    savedScripts,
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
