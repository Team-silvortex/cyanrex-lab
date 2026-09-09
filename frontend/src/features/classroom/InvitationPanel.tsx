import { useEffect, useState } from "react";

import { useConfirmedAction } from "../../components/useConfirmedAction";
import { getEngineUrl } from "../../config/runtime";
import { useI18n } from "../../i18n/context";
import { classroomRequest } from "./api";
import { canJoinClassroom, parseClassroomDiscovery, parseClassroomInvitation, type ClassroomDiscovery } from "./connection";

type InvitationView = { invite_id: string; username: string; expires_at: string };
type Invitation = InvitationView & { join_url: string };

export function InvitationPanel() {
  const { t } = useI18n();
  const confirmation = useConfirmedAction();
  const [open, setOpen] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [discovery, setDiscovery] = useState<ClassroomDiscovery | null>(null);
  const [invitations, setInvitations] = useState<InvitationView[]>([]);
  const [issued, setIssued] = useState<Invitation | null>(null);
  const [username, setUsername] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open) { setIssued(null); return; }
    const controller = new AbortController();
    setLoading(true); setError("");
    void Promise.all([
      classroomRequest<unknown>("/.well-known/cyanrex-classroom", undefined, controller.signal),
      classroomRequest<{ invitations: InvitationView[] }>("/classroom/invitations", undefined, controller.signal),
    ]).then(([descriptor, inventory]) => {
      const parsed = parseClassroomDiscovery(descriptor, window.location.origin, getEngineUrl());
      if (!parsed) throw new Error(t("classroom.unavailable"));
      if (!controller.signal.aborted) { setDiscovery(parsed); setInvitations(inventory.invitations); }
    }).catch(cause => {
      if (!controller.signal.aborted) { setDiscovery(null); setError((cause as Error).message); }
    }).finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [open, refresh, t]);

  const issue = () => {
    const target = username.trim().toLowerCase();
    if (!discovery || !/^[a-z0-9_.-]{3,64}$/.test(target) || confirmation.isBusy()) return;
    confirmation.request({ action: t("classroom.createInvitation"), description: t("classroom.inviteImpact"),
      targets: [discovery.display_name, discovery.classroom_id, target] }, async signal => {
      setIssued(null);
      const result = await classroomRequest<Invitation>("/classroom/invitations", { username: target }, signal);
      const url = new URL(result.join_url);
      if (url.origin + url.pathname !== discovery.join_url || url.search || result.username !== target
        || !canJoinClassroom(discovery, parseClassroomInvitation(url.hash))) throw new Error(t("classroom.unavailable"));
      if (!signal.aborted) { setIssued(result); setRefresh(value => value + 1); }
    });
  };
  const revoke = (invitation: InvitationView) => confirmation.request({
    action: t("classroom.revoke"), description: t("classroom.revokeImpact"), targets: [invitation.username, invitation.invite_id],
  }, async signal => {
    await classroomRequest("/classroom/invitations/revoke", { invite_id: invitation.invite_id }, signal);
    if (!signal.aborted) {
      setIssued(current => current?.invite_id === invitation.invite_id ? null : current);
      setRefresh(value => value + 1);
    }
  });

  return <details className="panel" style={{ marginTop: 16, overflowWrap: "anywhere" }} onToggle={event => setOpen(event.currentTarget.open)}>
    <summary>{t("classroom.manageInvitations")}</summary>
    {open && <div style={{ marginTop: 14 }}>
      <p className="meta">{t("classroom.manageHint")}</p>
      {error && <p className="error" role="alert">{error}</p>}
      {discovery && <p className="meta">{discovery.display_name} · <a href={discovery.join_url}>{t("classroom.publicEntry")}</a></p>}
      <form className="row" style={{ flexWrap: "wrap" }} onSubmit={event => { event.preventDefault(); issue(); }}>
        <label className="field-label">{t("classroom.studentUsername")}<input value={username} onChange={event => setUsername(event.target.value)} minLength={3} maxLength={64} pattern="[a-zA-Z0-9_.-]+" required disabled={confirmation.busy} /></label>
        <button type="submit" disabled={loading || !discovery || confirmation.busy}>{t("classroom.createInvitation")}</button>
        <button type="button" className="button-secondary" disabled={loading || confirmation.busy} onClick={() => setRefresh(value => value + 1)}>{t("common.refresh")}</button>
      </form>
      {issued && <div className="panel" style={{ marginTop: 12 }}>
        <p>{t("classroom.sharePrivately")}</p>
        <label className="field-label">{t("classroom.invitationLink")}<textarea aria-label={t("classroom.invitationLink")} readOnly rows={4} value={issued.join_url} onFocus={event => event.target.select()} /></label>
        <p className="meta">{t("classroom.expires", { time: new Date(issued.expires_at).toLocaleString() })}</p>
      </div>}
      <ul style={{ paddingLeft: 20 }}>{invitations.map(invitation => <li key={invitation.invite_id} style={{ marginTop: 12 }}>
        <span>{invitation.username} · {t("classroom.expires", { time: new Date(invitation.expires_at).toLocaleString() })} </span>
        <button type="button" className="button-secondary" disabled={confirmation.busy} onClick={() => revoke(invitation)}>{t("classroom.revoke")}</button>
      </li>)}</ul>
    </div>}
    {confirmation.dialog}
  </details>;
}
