import assert from "node:assert/strict";
import test from "node:test";

import {
  compareClientSurfaces,
  extractClientSurface,
  renderClientSurfaceBaseline,
} from "../sdk-surface-compatibility.mjs";

test("client surface extraction keeps public methods and nested namespace paths", () => {
  const source = `
export class CyanrexClient {
  readonly baseUrl: string;
  readonly version = "1.0.0";
  private readonly fetcher: unknown;

  constructor() {}

  readonly auth = {
    login: () => "{not a brace}",
    totp: {
      bootstrap: () => true,
    },
  };

  async operation<Name extends string>(name: Name) {}
  request<T>(method: string): Promise<T> { throw new Error(method); }
  private get<T>() {}
}
`;

  assert.deepEqual(extractClientSurface(source), [
    "auth",
    "auth.login",
    "auth.totp",
    "auth.totp.bootstrap",
    "baseUrl",
    "constructor",
    "operation",
    "request",
    "version",
  ]);
});

test("client surface comparison rejects removals while reporting additive members", () => {
  assert.deepEqual(
    compareClientSurfaces(
      ["auth", "auth.login", "events.list"],
      ["auth", "auth.login", "events.export"],
    ),
    {
      missing: ["events.list"],
      added: ["events.export"],
    },
  );
  assert.deepEqual(
    compareClientSurfaces(["auth", "auth.login"], ["auth", "auth.login", "auth.logout"]),
    { missing: [], added: ["auth.logout"] },
  );
});

test("client surface baseline rendering is sorted and deterministic", () => {
  const first = renderClientSurfaceBaseline(["system.info", "system", "system.info"], "0.3.1");
  const second = renderClientSurfaceBaseline(["system.info", "system"], "0.3.1");

  assert.equal(first, second);
  assert.deepEqual(JSON.parse(first), {
    schemaVersion: 1,
    policy: "additive-only",
    sourceVersion: "0.3.1",
    client: "CyanrexClient",
    members: ["system", "system.info"],
  });
});
