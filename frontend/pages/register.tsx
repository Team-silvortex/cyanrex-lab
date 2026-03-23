import Link from "next/link";
import { FormEvent, useMemo, useState } from "react";

import { sanitizeForDisplay } from "../src/utils/security";

type RegisterResponse = {
  ok: boolean;
  message: string;
  issuer?: string;
  account_name?: string;
  secret?: string;
  otpauth_uri?: string;
};

export default function RegisterPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [payload, setPayload] = useState<RegisterResponse | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setPayload(null);
    setQrDataUrl(null);

    if (password !== confirmPassword) {
      setError("两次输入密码不一致");
      return;
    }

    setLoading(true);
    try {
      const response = await fetch(`${engineUrl}/auth/register`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
      });

      const json = (await response.json()) as RegisterResponse;
      if (!response.ok || !json.ok || !json.otpauth_uri) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }

      setPayload(json);
      const qrcode = await import("qrcode");
      const dataUrl = await qrcode.toDataURL(json.otpauth_uri, {
        width: 280,
        margin: 1,
      });
      setQrDataUrl(dataUrl);
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
        <h1 style={{ marginTop: 6 }}>注册账号</h1>
        <p className="meta">创建账号后请立即绑定 OTP，登录必须使用 password + OTP。</p>

        <form onSubmit={onSubmit} style={{ marginTop: 14 }}>
          <div className="grid" style={{ gap: 10 }}>
            <input
              type="text"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder="username (>=3)"
              required
            />
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="password (>=8)"
              required
            />
            <input
              type="password"
              value={confirmPassword}
              onChange={(event) => setConfirmPassword(event.target.value)}
              placeholder="confirm password"
              required
            />
            <button type="submit" disabled={loading}>
              {loading ? "Creating..." : "Create Account"}
            </button>
          </div>
        </form>

        {error && <p className="error" style={{ marginTop: 12 }}>{sanitizeForDisplay(error)}</p>}

        {payload && (
          <div className="panel" style={{ marginTop: 14, background: "#0b1425" }}>
            <p className="meta" style={{ marginTop: 0 }}>
              账号创建成功：{payload.account_name}
            </p>
            {qrDataUrl && (
              <img
                src={qrDataUrl}
                alt="OTP QR code"
                style={{ width: 240, height: 240, borderRadius: 10, border: "1px solid #1d2f4f" }}
              />
            )}
            <p className="meta" style={{ marginTop: 10 }}>
              secret: <code>{payload.secret}</code>
            </p>
          </div>
        )}

        <p style={{ marginTop: 12 }}>
          <Link href="/login" className="meta">已有账号？返回登录</Link>
        </p>
      </section>
    </div>
  );
}
