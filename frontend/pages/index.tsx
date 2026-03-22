import Head from "next/head";
import { ChangeEvent, useMemo, useState } from "react";

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

export default function HomePage() {
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
    <>
      <Head>
        <title>Cyanrex eBPF Runner</title>
      </Head>
      <main style={{ maxWidth: 1100, margin: "0 auto", padding: 24, fontFamily: "sans-serif" }}>
        <h1>Cyanrex eBPF Runner</h1>
        <p>上传或编辑 eBPF C 代码，后端将编译并尝试加载，返回执行结果。</p>

        <div style={{ display: "flex", gap: 12, marginBottom: 12, flexWrap: "wrap" }}>
          <input type="file" accept=".c,.h,.txt" onChange={onUpload} />
          <button type="button" onClick={runEbpf} disabled={running}>
            {running ? "运行中..." : "编译并运行"}
          </button>
        </div>

        <textarea
          value={code}
          onChange={(event) => setCode(event.target.value)}
          style={{ width: "100%", minHeight: 340, fontFamily: "monospace", fontSize: 13, padding: 12 }}
          spellCheck={false}
        />

        <section style={{ marginTop: 20 }}>
          <h2>结果</h2>
          {!result && !error && <p>还没有运行结果。</p>}
          {error && <p style={{ color: "#b00020" }}>{error}</p>}
          {result && (
            <div style={{ border: "1px solid #ddd", borderRadius: 8, padding: 12 }}>
              <p><strong>success:</strong> {String(result.success)}</p>
              <p><strong>stage:</strong> {result.stage}</p>
              <p><strong>message:</strong> {result.message}</p>

              <h3>Compile Stdout</h3>
              <pre style={{ whiteSpace: "pre-wrap" }}>{result.compile_stdout || "(empty)"}</pre>

              <h3>Compile Stderr</h3>
              <pre style={{ whiteSpace: "pre-wrap" }}>{result.compile_stderr || "(empty)"}</pre>

              <h3>Load Stdout</h3>
              <pre style={{ whiteSpace: "pre-wrap" }}>{result.load_stdout || "(empty)"}</pre>

              <h3>Load Stderr</h3>
              <pre style={{ whiteSpace: "pre-wrap" }}>{result.load_stderr || "(empty)"}</pre>
            </div>
          )}
        </section>
      </main>
    </>
  );
}
