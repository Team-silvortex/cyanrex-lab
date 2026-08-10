import assert from "node:assert/strict";
import test from "node:test";

import securityHeaders from "../config/security-headers.js";

const {
  DEFAULT_ENGINE_ORIGIN,
  buildContentSecurityPolicy,
  resolveEngineOrigin,
} = securityHeaders;

test("CSP permits the configured Engine origin", () => {
  const policy = buildContentSecurityPolicy(" https://lab.example.test:8443/api ");
  assert.match(policy, /connect-src 'self' https:\/\/lab\.example\.test:8443 ws: wss:/);
  assert.doesNotMatch(policy, /\/api/);
});

test("invalid or non-http Engine URLs fall back without injecting CSP", () => {
  assert.equal(resolveEngineOrigin("javascript:alert(1)"), DEFAULT_ENGINE_ORIGIN);
  assert.equal(resolveEngineOrigin("https://lab.example.test; script-src *"), DEFAULT_ENGINE_ORIGIN);
  assert.match(buildContentSecurityPolicy("not a url"), /http:\/\/localhost:8080/);
});
