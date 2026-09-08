import assert from "node:assert/strict";
import test from "node:test";

import { startEventStream, mergeEventSnapshot } from "../src/features/events/eventStream.ts";

const event = (index, extra = {}) => ({
  username: "alice", timestamp: `2026-09-08T00:00:${String(index).padStart(2, "0")}Z`,
  source: "test", event_type: "test.event", category: "kernel", severity: "success",
  color: "green", payload: { index }, ...extra,
});
const flush = async () => { for (let n = 0; n < 8; n++) await Promise.resolve(); };

function harness(overrides = {}) {
  const sockets = [], requests = [], updates = [], states = [], gaps = [], timers = new Map();
  let clock = 0, timerId = 0;
  const dispose = startEventStream({
    socketUrl: "ws://localhost/ws/events", snapshotUrl: "http://localhost/events?limit=200",
    onEvents: (rows) => updates.push(rows), onState: (state) => states.push(state),
    onGap: () => gaps.push(true), ...overrides,
  }, {
    connect: () => {
      const socket = { closeCount: 0, close() { this.closeCount++; } };
      sockets.push(socket);
      return socket;
    },
    snapshot: (url, signal) => new Promise((resolve, reject) => requests.push({ url, signal, resolve, reject })),
    later: (callback, delay) => { timers.set(++timerId, { callback, delay }); return timerId; },
    cancel: (id) => timers.delete(id), now: () => clock, random: () => 0.5,
  });
  return {
    sockets, requests, updates, states, gaps, timers, dispose,
    open: () => sockets.at(-1).onopen(),
    message: (row) => sockets.at(-1).onmessage({ data: typeof row === "string" ? row : JSON.stringify(row) }),
    close: (code = 1013) => sockets.at(-1).onclose({ code }),
    tick: () => {
      assert.equal(timers.size, 1, "only one retry/deadline timer may be active");
      const [id, { callback, delay }] = [...timers][0];
      timers.delete(id); clock += delay; callback(); return delay;
    },
  };
}

test("subscribes before snapshot and merges in-flight rows without erasing live data", async () => {
  const h = harness();
  assert.equal(h.requests.length, 0);
  h.open();
  h.message(event(2)); h.message(event(3));
  h.requests[0].resolve([event(1), event(2)]);
  await flush();
  assert.deepEqual(h.updates.at(-1).map((row) => row.payload.index), [1, 2, 3]);
  assert.equal(h.states.at(-1), "open");
  h.message(event(4));
  assert.equal(h.updates.at(-1).at(-1).payload.index, 4);
  h.dispose();
});

test("snapshot merging keeps repeated identical events and insertion order", () => {
  const repeated = event(2);
  assert.deepEqual(mergeEventSnapshot([event(8), repeated, repeated], [repeated, repeated, event(1)], 4),
    [event(8), repeated, repeated, event(1)]);
  // JSONB may reorder object keys; do not mistake this for a different event.
  assert.equal(mergeEventSnapshot([event(1, { payload: { a: 1, b: 2 } })],
    [event(1, { payload: { b: 2, a: 1 } })], 200).length, 1);
});

test("frames already present in a snapshot may arrive after the HTTP response without duplication", async () => {
  const h = harness(); h.open();
  h.requests[0].resolve([event(1), event(2), event(2)]); await flush();
  h.message(event(2)); h.message(event(2));
  assert.deepEqual(h.updates.at(-1), [event(1), event(2), event(2)]);
  h.message(event(3)); h.message(event(2));
  assert.deepEqual(h.updates.at(-1), [event(1), event(2), event(2), event(3), event(2)]);
  h.dispose();
});

test("lag triggers a gap notice, backoff, and a fresh snapshot on reconnect", async () => {
  const h = harness();
  h.open(); h.requests[0].resolve([event(1)]); await flush();
  h.close();
  assert.equal(h.gaps.length, 1);
  assert.equal(h.states.at(-1), "recovering");
  assert.ok(h.tick() >= 500);
  h.open(); h.message(event(3));
  h.requests[1].resolve([event(1), event(2)]); await flush();
  assert.deepEqual(h.updates.at(-1).map((row) => row.payload.index), [1, 2, 3]);
  h.dispose();
});

test("old connection callbacks and late aborted snapshots cannot overwrite a new generation", async () => {
  const h = harness();
  h.open(); const old = h.sockets[0]; const oldMessage = old.onmessage;
  h.close(); assert.ok(h.requests[0].signal.aborted);
  h.tick(); h.open(); h.requests[1].resolve([event(3)]); await flush();
  h.requests[0].resolve([event(1)]); oldMessage({ data: JSON.stringify(event(2)) }); await flush();
  assert.deepEqual(h.updates.at(-1), [event(3)]);
  h.dispose();
});

test("snapshot buffer overflow is visible and bounded, including under a sustained burst", () => {
  const h = harness({ limit: 3 }); h.open();
  for (let n = 0; n < 10000; n++) h.message(event(n % 60));
  assert.equal(h.gaps.length, 1); assert.equal(h.updates.length, 0);
  assert.equal(h.requests.length, 1); assert.ok(h.requests[0].signal.aborted);
  assert.equal(h.sockets[0].closeCount, 1); assert.equal(h.timers.size, 1);
  h.dispose();
});

test("filtering applies to both snapshot and live rows before consuming the bounded buffer", async () => {
  const h = harness({ limit: 2, accepts: (row) => row.category === "kernel" });
  h.open();
  for (let n = 0; n < 100; n++) h.message(event(3, { category: "platform" }));
  h.requests[0].resolve([event(1, { category: "platform" }), event(2)]); await flush();
  h.message(event(3)); h.message(event(4));
  assert.deepEqual(h.updates.at(-1).map((row) => row.payload.index), [3, 4]);
  assert.equal(h.gaps.length, 0); h.dispose();
});

test("failed snapshot never reports open; retries do not overlap and stop on HTTP auth failure", async () => {
  const h = harness(); h.open(); h.requests[0].reject(new Error("network")); await flush();
  assert.equal(h.states.at(-1), "recovering"); h.tick(); h.open();
  h.requests[1].reject(Object.assign(new Error("unauthorized"), { status: 401 })); await flush();
  assert.equal(h.states.at(-1), "closed"); assert.equal(h.timers.size, 0); h.dispose();
});

test("handshake and snapshot deadlines cancel stalled work", () => {
  const h = harness(); assert.equal(h.tick(), 10000);
  assert.equal(h.sockets[0].closeCount, 1); h.tick(); h.open();
  assert.equal(h.tick(), 10000); assert.ok(h.requests[0].signal.aborted);
  assert.equal(h.states.at(-1), "recovering"); h.dispose();
});

test("repeated rapid disconnects back off to at most 30s instead of resetting at each handshake", async () => {
  const h = harness(); const delays = [];
  for (let n = 0; n < 12; n++) {
    h.open(); h.requests.at(-1).resolve([]); await flush(); h.close(); delays.push(h.tick());
  }
  assert.ok(delays.at(-1) >= 15000 && delays.at(-1) <= 30000);
  assert.ok(delays.every((delay, index) => !index || delay >= delays[index - 1])); h.dispose();
});

test("malformed frames and cleanup cannot mutate state or leave timers/requests alive", async () => {
  const h = harness(); h.open(); h.requests[0].resolve([]); await flush();
  const count = h.updates.length;
  for (const value of ["{", "null", "[]", "{}", JSON.stringify({ ...event(1), color: "invalid" })]) h.message(value);
  assert.equal(h.updates.length, count);
  h.dispose(); h.close(); h.message(event(1));
  assert.equal(h.timers.size, 0); assert.equal(h.updates.length, count);
});

test("disposal during snapshot aborts it; a new filter controller does not inherit its data", async () => {
  const old = harness(); old.open(); old.dispose();
  assert.ok(old.requests[0].signal.aborted);
  const current = harness({ accepts: (row) => row.severity === "error" }); current.open();
  current.requests[0].resolve([event(2, { severity: "error" })]); await flush();
  old.requests[0].resolve([event(1)]); await flush();
  assert.equal(old.updates.length, 0); assert.equal(old.timers.size, 0);
  assert.equal(current.updates.at(-1)[0].payload.index, 2); current.dispose();
});

test("an invalid snapshot retries; policy closure is terminal; limit one remains bounded", async () => {
  const h = harness({ limit: 1 }); h.open(); h.requests[0].resolve({ error: "bad response" }); await flush();
  assert.equal(h.states.at(-1), "recovering"); h.tick(); h.open();
  h.requests[1].resolve([event(1)]); await flush(); h.message(event(2));
  assert.deepEqual(h.updates.at(-1), [event(2)]);
  h.close(1008); assert.equal(h.states.at(-1), "closed"); assert.equal(h.timers.size, 0); h.dispose();
});
