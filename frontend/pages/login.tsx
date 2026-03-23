import { FormEvent, useMemo, useState } from "react";
import { useRouter } from "next/router";
import Link from "next/link";

import { sanitizeForDisplay } from "../src/utils/security";

type LoginResponse = {
  ok: boolean;
  message: string;
};

export default function LoginPage() {
  const router = useRouter();
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [otp, setOtp] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${engineUrl}/auth/login`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ username, password, otp }),
      });

      const json = (await response.json()) as LoginResponse;
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }

      const next = typeof router.query.next === "string" ? router.query.next : "/dashboard";
      router.replace(next);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="auth-shell">
      <section className="auth-card">
        <p className="brand-kicker">CYANREX</p>
        <h1 style={{ marginTop: 6 }}>Login</h1>
        <p className="meta">Password + OTP (TOTP) verification</p>

        <form onSubmit={onSubmit} style={{ marginTop: 14 }}>
          <div className="grid" style={{ gap: 10 }}>
            <input
              type="text"
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="username"
              required
            />
            <input
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="password"
              required
            />
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]{6}"
              value={otp}
              onChange={(e) => setOtp(e.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="6-digit OTP"
              required
            />
            <button type="submit" disabled={loading}>
              {loading ? "Signing in..." : "Sign In"}
            </button>
          </div>
        </form>

        {error && <p className="error" style={{ marginTop: 12 }}>{sanitizeForDisplay(error)}</p>}

        <p className="meta" style={{ marginTop: 12 }}>
          默认账号: <code>admin</code>，密码默认值见后端环境变量
          <code>CYANREX_ADMIN_PASSWORD</code>。
        </p>
        <div className="auth-otp-cta-wrap">
          <Link href="/otp-setup" className="auth-otp-cta">
            <span className="auth-otp-cta-kicker">OTP Setup</span>
            <strong>还没绑定 OTP？去生成绑定二维码</strong>
          </Link>
        </div>
        <p style={{ marginTop: 10 }}>
          <Link href="/register" className="meta">没有账号？去注册</Link>
        </p>
      </section>
    </div>
  );
}
