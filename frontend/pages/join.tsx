import Head from "next/head";
import Link from "next/link";
import { useRouter } from "next/router";
import { FormEvent, useEffect, useRef, useState } from "react";
import packageMetadata from "../package.json";

import LanguageSwitcher from "../src/components/LanguageSwitcher";
import { useConfirmedAction } from "../src/components/useConfirmedAction";
import { getEngineUrl } from "../src/config/runtime";
import { classroomRequest } from "../src/features/classroom/api";
import { canJoinClassroom, isClassroomTransportSafe, parseClassroomDiscovery, parseClassroomInvitation, CLASSROOM_PROTOCOL, type ClassroomDiscovery, type ClassroomInvitation } from "../src/features/classroom/connection";
import { useI18n } from "../src/i18n/context";

type Enrollment = { ok: boolean; account_name: string; secret: string; otpauth_uri: string };

export default function JoinPage() {
  const { t } = useI18n();
  const router = useRouter();
  const confirmation = useConfirmedAction();
  const captured = useRef<string | null>(null);
  const [invitation, setInvitation] = useState<ClassroomInvitation | null>(null);
  const [discovery, setDiscovery] = useState<ClassroomDiscovery | null>(null);
  const [origin, setOrigin] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  const [trusted, setTrusted] = useState(false);
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(true);
  const [enrollment, setEnrollment] = useState<Enrollment | null>(null);
  const [qr, setQr] = useState("");
  const engineUrl = getEngineUrl();

  useEffect(() => {
    if (!router.isReady || captured.current === router.asPath) return;
    captured.current = router.asPath;
    setInvitation(parseClassroomInvitation(window.location.hash));
    // Remove the bearer capability before any API request. Never persist it in browser storage.
    const cleanPath = window.location.pathname + window.location.search;
    window.history.replaceState({ ...window.history.state, url: cleanPath, as: cleanPath }, "", cleanPath);
    setOrigin(window.location.origin);
    setPassword(""); setRepeat(""); setTrusted(false); setEnrollment(null); setQr("");
  }, [router.isReady, router.asPath]);

  useEffect(() => {
    if (!origin) return;
    const controller = new AbortController();
    setChecking(true); setDiscovery(null); setError("");
    if (!isClassroomTransportSafe(origin) || !isClassroomTransportSafe(engineUrl)) {
      setError(t("classroom.unavailable")); setChecking(false); return;
    }
    void classroomRequest<unknown>("/.well-known/cyanrex-classroom", undefined, controller.signal).then(payload => {
      const parsed = parseClassroomDiscovery(payload, origin, engineUrl);
      if (!parsed) throw new Error(t("classroom.unavailable"));
      if (!controller.signal.aborted) setDiscovery(parsed);
    }).catch(cause => {
      if (!controller.signal.aborted) setError((cause as Error).message);
    }).finally(() => { if (!controller.signal.aborted) setChecking(false); });
    return () => controller.abort();
  }, [engineUrl, origin, t]);

  const compatible = canJoinClassroom(discovery, invitation);
  const submit = (event: FormEvent) => {
    event.preventDefault(); setError("");
    if (!compatible || !invitation || !discovery || !trusted || enrollment || confirmation.isBusy()) return;
    if (password !== repeat) { setError(t("auth.passwordMismatch")); return; }
    const body = { classroom_id: discovery.classroom_id, protocol_version: CLASSROOM_PROTOCOL, client_version: packageMetadata.version,
      required_capabilities: ["student-invite-v1"], invite_token: invitation.token, username: username.trim().toLowerCase(), password };
    confirmation.request({ action: t("classroom.join"), description: t("classroom.joinImpact"),
      targets: [discovery.display_name, origin, engineUrl, discovery.classroom_id, body.username], dangerous: false }, async signal => {
      // Clear secrets even after an uncertain response; a consumed invitation is never retried automatically.
      setInvitation(null); setPassword(""); setRepeat("");
      const result = await classroomRequest<Enrollment>("/classroom/join", body, signal);
      if (!result.ok || !result.secret || !result.otpauth_uri) throw new Error(t("classroom.unavailable"));
      if (signal.aborted) return;
      setEnrollment(result);
      // A QR rendering failure must not hide the successfully returned recovery secret.
      try {
        const qrcode = await import("qrcode");
        const data = await qrcode.toDataURL(result.otpauth_uri, { width: 240, margin: 1 });
        if (!signal.aborted) setQr(data);
      } catch { /* The Base32 secret remains visible for manual authenticator setup. */ }
    });
  };

  return <div className="auth-shell">
    <Head><title>{t("classroom.join")}</title><meta name="referrer" content="no-referrer" /></Head>
    <section className="auth-card" style={{ overflowWrap: "anywhere" }}>
      <div className="row" style={{ justifyContent: "space-between" }}><h1>{t("classroom.join")}</h1><LanguageSwitcher compact /></div>
      <p className="meta">{t("classroom.discoveryHint")}</p>
      {checking && <p role="status">{t("common.checking")}</p>}
      {discovery && <div className="panel" data-testid="classroom-identity">
        <h2>{discovery.display_name}</h2>
        <p>{origin}</p><p className="meta">API: {engineUrl}</p><code>{discovery.classroom_id}</code>
        <p className="meta">{t("classroom.versions", { version: discovery.product_version, min: discovery.protocol_min, max: discovery.protocol_max })}</p>
      </div>}
      {error && <p className="error" role="alert">{error}</p>}
      {!checking && discovery && !enrollment && !compatible && <p role="alert">{invitation ? t("classroom.incompatible") : t("classroom.needInvitation")}</p>}
      {compatible && !enrollment && <form onSubmit={submit} className="grid" style={{ marginTop: 16, gap: 10 }}>
        <label className="field-label">{t("auth.username")}<input autoComplete="username" value={username} onChange={event => setUsername(event.target.value)} minLength={3} maxLength={64} pattern="[a-zA-Z0-9_.-]+" required disabled={confirmation.busy} /></label>
        <label className="field-label">{t("auth.password")}<input type="password" autoComplete="new-password" value={password} onChange={event => setPassword(event.target.value)} minLength={8} maxLength={256} required disabled={confirmation.busy} /></label>
        <label className="field-label">{t("auth.confirmPassword")}<input type="password" autoComplete="new-password" value={repeat} onChange={event => setRepeat(event.target.value)} required disabled={confirmation.busy} /></label>
        <label style={{ display: "flex", gap: 10, alignItems: "start" }}><input type="checkbox" style={{ width: "auto", flex: "none" }} checked={trusted} onChange={event => setTrusted(event.target.checked)} disabled={confirmation.busy} />{t("classroom.confirmTeacher")}</label>
        <button disabled={!trusted || confirmation.busy} type="submit">{t("classroom.join")}</button>
      </form>}
      {enrollment && <div className="panel" data-testid="classroom-enrolled">
        <p>{t("auth.accountCreated", { account: enrollment.account_name })}</p><p>{t("classroom.saveOtp")}</p>
        {qr && <img src={qr} width={240} height={240} alt={t("auth.totpQrCode")} style={{ maxWidth: "100%", height: "auto" }} />}
        <p>{t("auth.secretLabel")}: <code>{enrollment.secret}</code></p>
      </div>}
      <p><Link href="/login">{t("auth.backToLogin")}</Link></p>
      {confirmation.dialog}
    </section>
  </div>;
}
