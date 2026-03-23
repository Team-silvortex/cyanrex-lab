import dynamic from "next/dynamic";
import { loader } from "@monaco-editor/react";
import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { analyzeCCode } from "../src/utils/cAnalyzer";
import { registerEbpfIntelligence } from "../src/utils/cEbpfIntelligence";
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
  const [code, setCode] = useState(SAMPLE_EBPF);
  const [result, setResult] = useState<EbpfRunResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [injectedMetadata, setInjectedMetadata] = useState<SelectedHeaderMetadata[]>([]);
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

    setRunning(true);
    setError(null);
    setResult(null);

    try {
      const response = await fetch(`${engineUrl}/ebpf/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ code }),
      });

      const json = (await response.json()) as EbpfRunResponse;
      setResult(json);

      if (!response.ok) {
        setError(`HTTP ${response.status}: ${json.message}`);
      }
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setRunning(false);
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
    <SidebarLayout title="Cyanrex eBPF Runner">
      <section className="panel">
        <h2>eBPF Runner (Light clangd mode)</h2>
        <p className="meta">Monaco + 常用 C 规则诊断 + 内联元数据（非完整 clangd）。</p>

        <div className="row" style={{ marginTop: 12 }}>
          <input type="file" accept=".c,.h,.txt" onChange={onUpload} />
          <button type="button" onClick={runEbpf} disabled={running}>
            {running ? "Running..." : "Compile & Run"}
          </button>
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
        <h3 style={{ marginTop: 0 }}>Inline Metadata</h3>
        <div className="row" style={{ marginBottom: 8 }}>
          <button type="button" onClick={refreshInjectedMetadata}>Refresh Injected Headers</button>
        </div>

        <p className="meta">lines: {analysis.metadata.lines} | bytes: {analysis.metadata.bytes}</p>
        <p className="meta">includes: {analysis.metadata.includes.join(", ") || "(none)"}</p>
        <p className="meta">injected includes: {analysis.metadata.injectedIncludes.join(", ") || "(none)"}</p>
        <p className="meta">sections: {analysis.metadata.sections.map((s) => `${s.name}@L${s.line}`).join(", ") || "(none)"}</p>
        <p className="meta">functions: {analysis.metadata.functions.map((f) => `${f.name}@L${f.line}`).join(", ") || "(none)"}</p>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>Injected Headers</h3>
        {injectedMetadata.length === 0 && <p className="meta">No selected header metadata.</p>}
        {injectedMetadata.map((item) => (
          <p key={item.id} className="meta">
            {item.include_hint} - {item.local_path}
          </p>
        ))}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>Diagnostics</h3>
        {analysis.diagnostics.length === 0 && <p className="meta">No diagnostics.</p>}
        {analysis.diagnostics.map((d, idx) => (
          <p key={`${d.line}-${idx}`} className={d.severity === "error" ? "error" : "meta"}>
            [{d.severity.toUpperCase()}] L{d.line}:{d.column} {d.message}
          </p>
        ))}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3>Result</h3>
        {!result && !error && <p className="meta">No run result yet.</p>}
        {error && <p className="error">{sanitizeForDisplay(error)}</p>}
        {result && (
          <>
            <p><strong>success:</strong> {String(result.success)}</p>
            <p><strong>stage:</strong> {sanitizeForDisplay(result.stage)}</p>
            <p><strong>message:</strong> {sanitizeForDisplay(result.message)}</p>

            <h4>Compile Stdout</h4>
            <pre>{sanitizeForDisplay(result.compile_stdout || "(empty)")}</pre>

            <h4>Compile Stderr</h4>
            <pre>{sanitizeForDisplay(result.compile_stderr || "(empty)")}</pre>

            <h4>Load Stdout</h4>
            <pre>{sanitizeForDisplay(result.load_stdout || "(empty)")}</pre>

            <h4>Load Stderr</h4>
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
