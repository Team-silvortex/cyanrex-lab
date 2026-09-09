export const CLASSROOM_PROTOCOL: number;
export type ClassroomInvitation = { token: string; classroomId: string; protocol: number };
export type ClassroomDiscovery = {
  service: string; classroom_id: string; display_name: string; product_version: string;
  protocol_min: number; protocol_max: number; join_url: string; capabilities: string[];
};
export function parseClassroomInvitation(hash: string): ClassroomInvitation | null;
export function isClassroomTransportSafe(value: string): boolean;
export function parseClassroomDiscovery(value: unknown, pageOrigin: string, engineUrl: string): ClassroomDiscovery | null;
export function canJoinClassroom(discovery: ClassroomDiscovery | null, invitation: ClassroomInvitation | null): boolean;
