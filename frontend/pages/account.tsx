import { FormEvent, useMemo, useState } from "react";
import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { sanitizeForDisplay } from "../src/utils/security";

export default function AccountPage() {
  const router = useRouter();
  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [otpForPassword, setOtpForPassword] = useState("");
  const [passwordMessage, setPasswordMessage] = useState<string | null>(null);
  const [passwordError, setPasswordError] = useState<string | null>(null);

  const [deletePassword, setDeletePassword] = useState("");
  const [deleteOtp, setDeleteOtp] = useState("");
  const [deleteConfirm, setDeleteConfirm] = useState("");
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const changePassword = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setPasswordError(null);
    setPasswordMessage(null);

    try {
      const response = await fetch(`${engineUrl}/auth/password/change`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
          otp: otpForPassword,
        }),
      });

      const json = (await response.json()) as { ok?: boolean; message?: string };
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }

      setPasswordMessage("密码已更新");
      setCurrentPassword("");
      setNewPassword("");
      setOtpForPassword("");
    } catch (err) {
      setPasswordError((err as Error).message);
    }
  };

  const deleteAccount = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setDeleteError(null);

    if (deleteConfirm !== "DELETE") {
      setDeleteError("请输入 DELETE 进行确认");
      return;
    }

    try {
      const response = await fetch(`${engineUrl}/auth/delete`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          password: deletePassword,
          otp: deleteOtp,
        }),
      });

      const json = (await response.json()) as { ok?: boolean; message?: string };
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }

      router.replace("/register");
    } catch (err) {
      setDeleteError((err as Error).message);
    }
  };

  return (
    <SidebarLayout title="Cyanrex Account">
      <section className="panel">
        <h2>Account Security</h2>
        <p className="meta">通过 OTP + 当前密码 修改密码；删号同样需要 OTP 验证。</p>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>修改密码</h3>
        <form onSubmit={changePassword}>
          <div className="grid" style={{ gap: 10 }}>
            <input
              type="password"
              value={currentPassword}
              onChange={(event) => setCurrentPassword(event.target.value)}
              placeholder="当前密码"
              required
            />
            <input
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
              placeholder="新密码（>=8）"
              required
            />
            <input
              type="text"
              value={otpForPassword}
              onChange={(event) => setOtpForPassword(event.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="6位 OTP"
              required
            />
            <button type="submit">更新密码</button>
          </div>
        </form>
        {passwordMessage && <p className="meta" style={{ marginTop: 10 }}>{passwordMessage}</p>}
        {passwordError && <p className="error" style={{ marginTop: 10 }}>{sanitizeForDisplay(passwordError)}</p>}
      </section>

      <section className="panel" style={{ marginTop: 16, borderColor: "#6b2b39" }}>
        <h3 style={{ marginTop: 0 }}>删除账号</h3>
        <p className="meta">危险操作：删除后该账号不可恢复。</p>
        <form onSubmit={deleteAccount}>
          <div className="grid" style={{ gap: 10 }}>
            <input
              type="password"
              value={deletePassword}
              onChange={(event) => setDeletePassword(event.target.value)}
              placeholder="账号密码"
              required
            />
            <input
              type="text"
              value={deleteOtp}
              onChange={(event) => setDeleteOtp(event.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="6位 OTP"
              required
            />
            <input
              type="text"
              value={deleteConfirm}
              onChange={(event) => setDeleteConfirm(event.target.value)}
              placeholder='输入 DELETE 确认'
              required
            />
            <button type="submit" style={{ background: "linear-gradient(130deg, #662430, #a1394c)", borderColor: "#8b3c4b" }}>
              删除账号
            </button>
          </div>
        </form>
        {deleteError && <p className="error" style={{ marginTop: 10 }}>{sanitizeForDisplay(deleteError)}</p>}
      </section>
    </SidebarLayout>
  );
}
