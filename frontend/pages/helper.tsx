import { useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";

type EnvironmentCheckItem = {
  name: string;
  ok: boolean;
  detail: string;
};

type EnvironmentReport = {
  overall_ok: boolean;
  generated_at: string;
  checks: EnvironmentCheckItem[];
};

export default function HelperPage() {
  const [report, setReport] = useState<EnvironmentReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const runCheck = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${engineUrl}/helper/environment`, {
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const json = (await response.json()) as EnvironmentReport;
      setReport(json);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <SidebarLayout title="Cyanrex Helper">
      <section className="panel">
        <h2>Host Environment Helper</h2>
        <p className="meta">检查主机运行 eBPF 所需环境版本与能力，避免“奇怪跑不起来”。</p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={runCheck} disabled={loading}>
            {loading ? "Checking..." : "Run Environment Check"}
          </button>
        </div>

        {error && <p className="error" style={{ marginTop: 12 }}>{error}</p>}

        {report && (
          <div style={{ marginTop: 16 }}>
            <p>
              <strong>overall:</strong> {report.overall_ok ? "OK" : "NOT READY"}
            </p>
            <p className="meta">
              generated_at: {new Date(report.generated_at).toLocaleString()}
            </p>

            <div className="grid" style={{ marginTop: 10 }}>
              {report.checks.map((check) => (
                <article key={check.name} className="panel" style={{ background: "#0b1425" }}>
                  <p>
                    <strong>{check.name}</strong>: {check.ok ? "OK" : "FAIL"}
                  </p>
                  <p className="meta" style={{ margin: 0 }}>{check.detail}</p>
                </article>
              ))}
            </div>
          </div>
        )}
      </section>
    </SidebarLayout>
  );
}
