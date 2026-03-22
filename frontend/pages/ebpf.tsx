import { ChangeEvent, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";

type EbpfRunResponse = {
  success: boolean;
  stage: string;
  message: string;
  compile_stdout: string;
  compile_stderr: string;
  load_stdout: string;
  load_stderr: string;
};

const SAMPLE_EBPF = `#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";`;

export default function EbpfPage() {
  const [code, setCode] = useState(SAMPLE_EBPF);
  const [result, setResult] = useState<EbpfRunResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const onUpload = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    setCode(text);
  };

  const runEbpf = async () => {
    setRunning(true);
    setError(null);
    setResult(null);

    try {
      const response = await fetch(`${engineUrl}/ebpf/run`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
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

  return (
    <SidebarLayout title="Cyanrex eBPF Runner">
      <section className="panel">
        <h2>eBPF Runner</h2>
        <p className="meta">上传或编辑 eBPF C 代码，后端编译加载并返回结果。</p>

        <div className="row" style={{ marginTop: 12 }}>
          <input type="file" accept=".c,.h,.txt" onChange={onUpload} />
          <button type="button" onClick={runEbpf} disabled={running}>
            {running ? "Running..." : "Compile & Run"}
          </button>
        </div>

        <div style={{ marginTop: 12 }}>
          <textarea
            value={code}
            onChange={(event) => setCode(event.target.value)}
            spellCheck={false}
          />
        </div>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3>Result</h3>
        {!result && !error && <p className="meta">No run result yet.</p>}
        {error && <p className="error">{error}</p>}
        {result && (
          <>
            <p><strong>success:</strong> {String(result.success)}</p>
            <p><strong>stage:</strong> {result.stage}</p>
            <p><strong>message:</strong> {result.message}</p>

            <h4>Compile Stdout</h4>
            <pre>{result.compile_stdout || "(empty)"}</pre>

            <h4>Compile Stderr</h4>
            <pre>{result.compile_stderr || "(empty)"}</pre>

            <h4>Load Stdout</h4>
            <pre>{result.load_stdout || "(empty)"}</pre>

            <h4>Load Stderr</h4>
            <pre>{result.load_stderr || "(empty)"}</pre>
          </>
        )}
      </section>
    </SidebarLayout>
  );
}
