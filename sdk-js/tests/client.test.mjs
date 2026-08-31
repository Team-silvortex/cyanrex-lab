import assert from "node:assert/strict";
import test from "node:test";

import { CyanrexApiError, CyanrexClient } from "../src/index.ts";

test("uses cookie credentials and serializes JSON requests", async () => {
  const calls = [];
  const client = new CyanrexClient("http://localhost:8080///", {
    fetch: async (url, init) => {
      calls.push({ url, init });
      return Response.json({ name: "module-network", status: "running" });
    },
  });

  const result = await client.modules.start("module-network");

  assert.deepEqual(result, { name: "module-network", status: "running" });
  assert.equal(calls[0].url, "http://localhost:8080/modules/start");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.credentials, "include");
  assert.equal(calls[0].init.headers["Content-Type"], "application/json");
  assert.equal(calls[0].init.body, JSON.stringify({ name: "module-network" }));
});

test("builds queries and sends command bus payloads", async () => {
  const calls = [];
  const client = new CyanrexClient("http://localhost:8080", {
    fetch: async (url, init) => {
      calls.push({ url, init });
      return Response.json({
        ok: true,
        commandType: "ListModules",
        message: "modules listed",
        modules: [],
      });
    },
  });

  await client.events.list({ category: "kernel", limit: 25 });
  await client.command.dispatch({ commandType: "ListModules" });

  assert.equal(calls[0].url, "http://localhost:8080/events?category=kernel&limit=25");
  assert.equal(calls[0].init.method, "GET");
  assert.equal(calls[0].init.credentials, "include");
  assert.equal(calls[1].init.body, JSON.stringify({ commandType: "ListModules" }));
});

test("throws a typed API error with parsed response details", async () => {
  const client = new CyanrexClient("http://localhost:8080", {
    fetch: async () => Response.json(
      { ok: false, message: "module name is required" },
      { status: 400 },
    ),
  });

  await assert.rejects(
    () => client.command.dispatch({ commandType: "StopModule" }),
    (error) => {
      assert.ok(error instanceof CyanrexApiError);
      assert.equal(error.status, 400);
      assert.equal(error.message, "module name is required");
      assert.deepEqual(error.details, { ok: false, message: "module name is required" });
      return true;
    },
  );
});

test("supports empty responses and caller cancellation signals", async () => {
  const calls = [];
  const client = new CyanrexClient("http://localhost:8080", {
    fetch: async (url, init) => {
      calls.push({ url, init });
      return new Response(null, { status: 204 });
    },
  });
  const controller = new AbortController();

  const result = await client.auth.logout({ signal: controller.signal });

  assert.equal(result, undefined);
  assert.equal(calls[0].init.signal, controller.signal);
});

test("captures a Node session cookie and adds the configured CSRF origin", async () => {
  const calls = [];
  const responses = [
    Response.json(
      {
        ok: true,
        message: "login success",
        username: "admin",
        role: "admin",
        expires_at: "2026-08-31T00:00:00Z",
      },
      { headers: { "set-cookie": "cyanrex_session=session-token; Path=/; HttpOnly" } },
    ),
    Response.json({ ok: true, commandType: "ListModules", message: "listed", modules: [] }),
  ];
  const client = new CyanrexClient("http://localhost:8080", {
    csrfOrigin: "http://localhost:3000/",
    fetch: async (url, init) => {
      calls.push({ url, init });
      return responses.shift();
    },
  });

  await client.auth.login({ username: "admin", password: "secret", otp: "123456" });
  await client.command.dispatch({ commandType: "ListModules" });

  assert.equal(client.getSessionCookie(), "cyanrex_session=session-token");
  assert.equal(calls[1].init.headers.Cookie, "cyanrex_session=session-token");
  assert.equal(calls[1].init.headers.Origin, "http://localhost:3000");
});

test("loads the public OpenAPI contract through the system namespace", async () => {
  const calls = [];
  const client = new CyanrexClient("http://localhost:8080", {
    fetch: async (url, init) => {
      calls.push({ url, init });
      return Response.json({ openapi: "3.1.0", info: { version: "0.2.9" }, paths: {} });
    },
  });

  const contract = await client.system.openapi();

  assert.equal(contract.openapi, "3.1.0");
  assert.equal(calls[0].url, "http://localhost:8080/openapi.json");
  assert.equal(calls[0].init.method, "GET");
});
