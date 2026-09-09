import assert from "node:assert/strict";
import test from "node:test";
import { parseClassroomInvitation, parseClassroomDiscovery, canJoinClassroom } from "../src/features/classroom/connection.js";

const origin = "https://classroom.example";
const id = "23d40d83-19de-43d3-85fe-506d86592a70";
const descriptor = { service: "cyanrex-classroom", classroom_id: id, display_name: "Lab A", product_version: "0.3.99",
  protocol_min: 1, protocol_max: 1, join_url: `${origin}/join`, capabilities: ["student-invite-v1"] };
const hash = `#invite=${"a".repeat(64)}&classroom=${id}&protocol=1`;

test("classroom invitation fragments are strictly parsed without accepting query/redirect overrides", () => {
  assert.deepEqual(parseClassroomInvitation(hash), { token: "a".repeat(64), classroomId: id, protocol: 1 });
  for (const invalid of ["", hash.replace("#", "?"), `${hash}&invite=${"b".repeat(64)}`, `${hash}&next=https://evil.example`, hash.replace("protocol=1", "protocol=-1"), hash.replace(id, "../teacher")]) {
    assert.equal(parseClassroomInvitation(invalid), null, invalid);
  }
});

test("classroom discovery is data, not a redirect or authority to switch API origins", () => {
  assert.deepEqual(parseClassroomDiscovery(descriptor, origin, "https://api.classroom.example"), descriptor);
  for (const join_url of ["https://evil.example/join", `${origin}/join?invite=secret`, `${origin}/join#secret`, `${origin}/other`, `https://user:pass@classroom.example/join`]) {
    assert.equal(parseClassroomDiscovery({ ...descriptor, join_url }, origin, "https://api.classroom.example"), null);
  }
  assert.equal(parseClassroomDiscovery(descriptor, origin, "http://192.168.1.9:8080"), null);
  assert.equal(parseClassroomDiscovery(descriptor, "http://classroom.example", "https://api.classroom.example"), null);
  const local = { ...descriptor, join_url: "http://localhost:3217/join" };
  assert.ok(parseClassroomDiscovery(local, "http://localhost:3217", "http://127.0.0.1:8080"));
  assert.ok(parseClassroomDiscovery(local, "http://localhost:3217", "http://127.0.0.2:8080"));
  assert.equal(parseClassroomDiscovery(local, "http://localhost:3217", "http://127.attacker.example:8080"), null);
});

test("join compatibility uses protocol and required capability, never matching patch strings", () => {
  const invitation = parseClassroomInvitation(hash);
  assert.equal(canJoinClassroom(descriptor, invitation), true);
  for (const incompatible of [{ ...descriptor, protocol_min: 2 }, { ...descriptor, protocol_max: 0 }, { ...descriptor, classroom_id: "another-classroom" }, { ...descriptor, capabilities: [] }]) {
    assert.equal(canJoinClassroom(incompatible, invitation), false);
  }
  assert.equal(canJoinClassroom(descriptor, { ...invitation, protocol: 2 }), false);
  assert.equal(canJoinClassroom(null, invitation), false);
  assert.equal(canJoinClassroom(descriptor, null), false);
});
