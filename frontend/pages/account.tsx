import { FormEvent, useMemo, useState } from "react";
import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { sanitizeForDisplay } from "../src/utils/security";

export default function AccountPage() {
  const { t } = useI18n();
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

      setPasswordMessage(t("account.passwordUpdated"));
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
      setDeleteError(t("account.deleteConfirmMismatch"));
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
    <SidebarLayout title={t("account.title")}>
      <section className="panel">
        <h2>{t("account.title")}</h2>
        <p className="meta">{t("account.subtitle")}</p>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3 style={{ marginTop: 0 }}>{t("account.changePassword")}</h3>
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
            <button type="submit">{t("account.updatePassword")}</button>
          </div>
        </form>
        {passwordMessage && <p className="meta" style={{ marginTop: 10 }}>{passwordMessage}</p>}
        {passwordError && <p className="error" style={{ marginTop: 10 }}>{sanitizeForDisplay(passwordError)}</p>}
      </section>

      <section className="panel" style={{ marginTop: 16, borderColor: "#6b2b39" }}>
        <h3 style={{ marginTop: 0 }}>{t("account.deleteAccount")}</h3>
        <p className="meta">{t("account.dangerous")}</p>
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
              placeholder={t("account.confirmDeleteHint")}
              required
            />
            <button type="submit" style={{ background: "linear-gradient(130deg, #662430, #a1394c)", borderColor: "#8b3c4b" }}>
              {t("account.deleteAction")}
            </button>
          </div>
        </form>
        {deleteError && <p className="error" style={{ marginTop: 10 }}>{sanitizeForDisplay(deleteError)}</p>}
      </section>
    </SidebarLayout>
  );
}
