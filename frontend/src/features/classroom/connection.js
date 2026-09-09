export const CLASSROOM_PROTOCOL = 1;
const idPattern = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;

export function parseClassroomInvitation(hash) {
  if (typeof hash !== "string" || !hash.startsWith("#") || hash.length > 256) return null;
  const values = new URLSearchParams(hash.slice(1));
  const keys = [...values.keys()];
  if (keys.length !== 3 || new Set(keys).size !== 3 || keys.some(key => !["invite", "classroom", "protocol"].includes(key))) return null;
  const token = values.get("invite"), classroomId = values.get("classroom"), protocol = values.get("protocol");
  if (!/^[a-f0-9]{64}$/.test(token ?? "") || !idPattern.test(classroomId ?? "") || !/^[1-9][0-9]{0,4}$/.test(protocol ?? "")) return null;
  return { token, classroomId, protocol: Number(protocol) };
}

export function isClassroomTransportSafe(value) {
  try {
    const url = new URL(value);
    const loopback = ["localhost", "[::1]"].includes(url.hostname) || /^127(?:\.\d{1,3}){3}$/.test(url.hostname);
    return !url.username && !url.password && !url.search && !url.hash && url.pathname === "/"
      && (url.protocol === "https:" || url.protocol === "http:" && loopback);
  } catch { return false; }
}

export function parseClassroomDiscovery(value, pageOrigin, engineUrl) {
  if (!isClassroomTransportSafe(pageOrigin) || !isClassroomTransportSafe(engineUrl)) return null;
  if (!value || typeof value !== "object" || value.service !== "cyanrex-classroom" || !idPattern.test(value.classroom_id)
    || typeof value.display_name !== "string" || !value.display_name.trim() || [...value.display_name].length > 64 || /[\u0000-\u001f\u007f]/.test(value.display_name)
    || typeof value.product_version !== "string" || !/^[a-zA-Z0-9.+-]{1,64}$/.test(value.product_version)
    || !Number.isInteger(value.protocol_min) || !Number.isInteger(value.protocol_max) || value.protocol_min < 1 || value.protocol_max < value.protocol_min
    || value.protocol_max > 65535 || !Array.isArray(value.capabilities) || value.capabilities.length > 16
    || value.capabilities.some(item => typeof item !== "string" || item.length > 64)) return null;
  // The discovery document can describe the current teacher, never select another API origin.
  if (value.join_url !== `${new URL(pageOrigin).origin}/join`) return null;
  return value;
}

export function canJoinClassroom(discovery, invitation) {
  return Boolean(discovery && invitation && discovery.classroom_id === invitation.classroomId
    && invitation.protocol === CLASSROOM_PROTOCOL && discovery.protocol_min <= CLASSROOM_PROTOCOL
    && discovery.protocol_max >= CLASSROOM_PROTOCOL && discovery.capabilities.includes("student-invite-v1"));
}
