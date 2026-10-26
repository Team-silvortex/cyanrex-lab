import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import {
  setup, waitFor, SESSION, LOGIN, LOGOUT, authenticated, signedOut, invalidSession,
  password, otp, username,
} from "./platformSessionUi/browserFixture.mjs";

// Real Chrome + production closed static assets and CSP; all Session API responses below are
// explicitly mocked. These cases are not real authentication, database, TLS or rollback evidence.
let browser;
before(async () => {
  assert.ok(process.env.CYANREX_PLAYWRIGHT_MODULE, "set CYANREX_PLAYWRIGHT_MODULE explicitly");
  assert.ok(process.env.CYANREX_CHROMIUM_PATH, "set CYANREX_CHROMIUM_PATH explicitly");
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE);
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

async function run(options, callback) {
  const f = await setup(browser, options);
  try { await callback(f); assert.deepEqual(f.violations, []); }
  finally { await f.close(); }
}
async function open(f, expected = "unauthenticated") {
  await f.page.goto(`${f.origin}/platform/`, { waitUntil: "domcontentloaded" });
  if (expected) { await f.state(expected); await f.ready(); }
}
const idle = f => f.page.waitForFunction(() => document.body.dataset.busy === "false");

test("blocked external JS leaves controls disabled and cannot send password/OTP in a GET or form fallback", { timeout: 15_000 }, () => run({ blockScript: true }, async f => {
  await open(f, null);
  for (const id of ["username", "password", "otp", "login", "check-session", "logout"]) {
    assert.equal(await f.page.locator(`#${id}`).isDisabled(), true);
  }
  await f.page.keyboard.press("Enter");
  await f.page.evaluate(({ password, otp }) => {
    document.querySelector("#password").value = password;
    document.querySelector("#otp").value = otp;
    // Test the fail-closed HTML/CSP fallback independently of the missing JS handler.
    document.querySelector("#session-form").requestSubmit();
  }, { password, otp });
  await f.page.waitForTimeout(80);
  assert.equal(f.apiRecords().length, 0);
  assert.equal(f.page.url(), `${f.origin}/platform/`);
  assert.ok(f.records.every(record => !record.url.includes(password) && !record.url.includes(otp)));
  assert.ok(f.logs.every(message => !message.includes(password) && !message.includes(otp)));
}));

test("fresh mocked 401 then strict login POST uses natural same-origin headers and one separate Cookie check", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, f.apiRecords(SESSION).length === 1 ? 401 : 200,
    f.apiRecords(SESSION).length === 1 ? invalidSession : authenticated);
  return f.respond(record, 200, authenticated);
} }, async f => {
  await open(f); await f.fill(); await f.page.locator("#login").click();
  await f.state("active"); await idle(f);
  assert.equal(f.apiRecords(LOGIN, "POST").length, 1);
  assert.equal(f.apiRecords(SESSION, "GET").length, 2);
  const request = f.apiRecords(LOGIN)[0];
  assert.deepEqual(JSON.parse(request.body), { schema_version: 1, username, password, otp });
  assert.equal(request.headers.origin, f.origin);
  assert.equal(request.headers["x-cyanrex-platform-request"], "1");
  assert.equal(request.headers["sec-fetch-site"], "same-origin");
  assert.equal(request.headers["sec-fetch-mode"], "same-origin");
  assert.equal(request.headers["content-type"], "application/json");
  assert.equal(request.headers.authorization, undefined);
  assert.ok(!await f.page.locator("#status").textContent().then(text => text.includes(username)));
  assert.equal(await f.page.locator("[data-role], [data-account-id]").count(), 0);
  await f.noLeaks();
}));

test("logout Cancel sends no POST and explicit confirmation dispatches once under double-click lock", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, authenticated);
  // Deliberately retain the mock logout response while testing its UI admission lock.
} }, async f => {
  await open(f, "active");
  await f.page.locator("#logout").click();
  assert.equal(await f.page.locator("#logout-dialog").evaluate(node => node.open), true);
  await f.page.locator("#cancel-logout").click(); assert.equal(f.apiRecords(LOGOUT).length, 0);
  await f.page.locator("#logout").click();
  await f.page.locator("#confirm-logout").evaluate(node => { node.click(); node.click(); });
  await waitFor(() => f.apiRecords(LOGOUT).length === 1);
  assert.equal(await f.page.locator("#login").isDisabled(), true);
  assert.equal(f.apiRecords(LOGOUT)[0].method, "POST");
  assert.equal(f.apiRecords(LOGOUT)[0].body, "");
  assert.equal(f.apiRecords(LOGOUT)[0].headers.origin, f.origin);
  f.respond(f.apiRecords(LOGOUT)[0], 200, signedOut);
  await f.state("unauthenticated"); await idle(f);
  await f.page.waitForTimeout(60); assert.equal(f.apiRecords(LOGOUT).length, 1);
  await f.noLeaks();
}));

test("mocked login errors, malformed/duplicate DTOs, overflow and redirects stay unknown without retry", { timeout: 30_000 }, async () => {
  const variants = [
    [503, '{"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}}'],
    [401, '{"schema_version":1,"error":{"code":"invalid_credentials","outcome":"unconfirmed"}}'],
    [200, "not JSON"], [200, '{"schema_version":1,"authenticated":false}'],
    [200, '{"schema_version":1,"authenticated":true,"authenticated":true}'],
    [200, '{"schema_version":1,"schema_version":1,"authenticated":true}'],
    [200, '{"schema_versi\\u006fn":1,"authenticated":true}'],
    [200, '{"schema_version":1,"authenticated":true,"role":"teacher"}'],
    [200, `${authenticated}${" ".repeat(4097)}`],
    [200, authenticated, { "content-type": "text/html" }],
    [302, "", { location: "/platform/v1/session?must-not-follow=1" }],
  ];
  for (const [status, raw, headers] of variants) await run({ api(record, f) {
    if (record.path === SESSION) return f.respond(record, 401, invalidSession);
    return f.respond(record, status, raw, headers);
  } }, async f => {
    await open(f); await f.fill(); await f.page.locator("#login").click();
    await f.state("unknown"); await idle(f); await f.page.waitForTimeout(30);
    assert.equal(f.apiRecords(LOGIN).length, 1);
    assert.equal(f.apiRecords(SESSION).length, 1, "an invalid login cannot trigger a follow-up read or redirect");
    await f.noLeaks();
  });
});

test("accelerated browser waiting timeout remains unknown and never replays or infers server rollback", { timeout: 15_000 }, () => run({ fastTimeout: true, api(record, f) {
  if (record.path === SESSION) return f.respond(record, 401, invalidSession);
  // Simulate a server mutation whose response is not delivered; no server rollback is modeled.
} }, async f => {
  await open(f); await f.fill(); await f.page.locator("#login").click();
  await waitFor(() => f.apiRecords(LOGIN).length === 1);
  await f.state("unknown"); await idle(f);
  f.respond(f.apiRecords(LOGIN)[0], 200, authenticated);
  await f.page.waitForTimeout(100);
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "unknown");
  assert.equal(f.apiRecords(LOGIN).length, 1); assert.equal(f.apiRecords(SESSION).length, 1);
  await f.noLeaks();
}));

test("existing mocked platform Cookie conflict is visible without replacement, logout or automatic re-login", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, 401, invalidSession);
  return f.respond(record, 409, '{"schema_version":1,"error":{"code":"session_present","outcome":"not_attempted"}}');
} }, async f => {
  await f.context.addCookies([{ name: "cyanrex_platform_session",
    value: "00000000-0000-4000-8000-000000000009", domain: "127.0.0.1", path: "/platform/v1",
    httpOnly: true, sameSite: "Strict" }]);
  const originalCookies = await f.context.cookies();
  await open(f); await f.fill(); await f.page.locator("#login").click();
  await f.state("session-present"); await idle(f); await f.page.waitForTimeout(50);
  assert.equal(f.apiRecords(LOGIN).length, 1); assert.equal(f.apiRecords(LOGOUT).length, 0);
  assert.equal(f.apiRecords(SESSION).length, 1);
  assert.equal(f.apiRecords(LOGIN)[0].headers.cookie, "cyanrex_platform_session=00000000-0000-4000-8000-000000000009");
  assert.deepEqual(await f.context.cookies(), originalCookies); await f.noLeaks();
}));

test("pagehide invalidates a previously active observation and persisted pageshow performs only a new read", { timeout: 15_000 }, () => run({ api(record, f) {
  if (f.apiRecords(SESSION).length === 1) return f.respond(record, 200, authenticated);
} }, async f => {
  await open(f, "active"); await f.fill();
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "unknown");
  assert.equal(await f.page.locator("#login").isDisabled(), true);
  assert.equal(await f.page.locator("#password").inputValue(), "");
  assert.equal(await f.page.locator("#otp").inputValue(), "");
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await waitFor(() => f.apiRecords(SESSION).length === 2);
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "checking");
  f.respond(f.apiRecords(SESSION)[1], 401, invalidSession);
  await f.state("unauthenticated"); await idle(f);
  assert.equal(f.apiRecords(LOGIN).length + f.apiRecords(LOGOUT).length, 0);
  await f.noLeaks();
}));

test("cancelled real HTTP from an old epoch cannot reactivate a persisted page after a new 401", { timeout: 15_000 }, () => run({ api() {} }, async f => {
  await open(f, null); await waitFor(() => f.apiRecords(SESSION).length === 1);
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "unknown");
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await waitFor(() => f.apiRecords(SESSION).length === 2);
  f.respond(f.apiRecords(SESSION)[1], 401, invalidSession);
  await f.state("unauthenticated"); await idle(f);
  // The old real HTTP connection may already be destroyed by abort: no late delivery is claimed.
  f.respond(f.apiRecords(SESSION)[0], 200, authenticated);
  await f.page.waitForTimeout(80);
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "unauthenticated");
  assert.equal(f.apiRecords(SESSION).length, 2); await f.noLeaks();
}));

test("controlled fetch delivers an old success after restored-page 401 without reviving the old epoch", { timeout: 15_000 }, () => run({ init: () => {
  const actual = window.fetch;
  window.__epochFetchCount = 0;
  window.__oldEpochResponseDelivered = false;
  window.__oldEpochBodyCancelled = false;
  window.fetch = function(path, options) {
    if (path === "/platform/v1/session" && ++window.__epochFetchCount === 1) {
      // Test-only substitution intentionally ignores options.signal; production request/epoch
      // code must handle a transport that resolves even after its caller has stopped waiting.
      return new Promise(resolve => {
        window.__deliverOldEpochResponse = () => {
          window.__oldEpochResponseDelivered = true;
          const body = new ReadableStream({
            start(controller) {
              controller.enqueue(new TextEncoder().encode('{"schema_version":1,"authenticated":true}'));
              controller.close();
            },
            cancel() { window.__oldEpochBodyCancelled = true; },
          });
          resolve(new Response(body, { status: 200, headers: { "content-type": "application/json" } }));
        };
      });
    }
    return actual.call(this, path, options);
  };
} }, async f => {
  await open(f, null);
  await f.page.waitForFunction(() => typeof window.__deliverOldEpochResponse === "function");
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await f.state("unknown");
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await f.state("unauthenticated"); await idle(f);
  assert.equal(f.apiRecords(SESSION).length, 1, "only the restored-page read uses the real mock HTTP server");
  await f.page.evaluate(() => window.__deliverOldEpochResponse());
  await f.page.waitForFunction(() => window.__oldEpochBodyCancelled === true);
  assert.equal(await f.page.evaluate(() => window.__oldEpochResponseDelivered), true);
  assert.equal(await f.page.evaluate(() => window.__epochFetchCount), 2);
  assert.equal(await f.page.locator("#status").getAttribute("data-state"), "unauthenticated");
  assert.equal(f.apiRecords(LOGIN).length + f.apiRecords(LOGOUT).length, 0);
  await f.noLeaks();
}));

test("a bounded synthetic ready-empty response stream cannot reach success or starve waiting forever", { timeout: 15_000 }, () => run({}, async f => {
  await open(f);
  await f.page.evaluate(() => {
    const actual = window.fetch; window.__emptyFrameCalls = 0;
    // Controlled transport substitution ONLY in this test; production app/readBounded is intact.
    window.fetch = function(path, options) {
      if (path !== "/platform/v1/session/login") return actual.call(this, path, options);
      window.__emptyFrameCalls++;
      let frames = 0;
      const body = new ReadableStream({ pull(controller) {
        if (++frames <= 5000) controller.enqueue(new Uint8Array());
        else { controller.enqueue(new TextEncoder().encode('{"schema_version":1,"authenticated":true}')); controller.close(); }
      } });
      return Promise.resolve(new Response(body, { status: 200, headers: { "content-type": "application/json" } }));
    };
  });
  await f.fill(); await f.page.locator("#login").click();
  await f.state("unknown"); await idle(f);
  assert.equal(await f.page.evaluate(() => window.__emptyFrameCalls), 1);
  assert.equal(f.apiRecords(SESSION).length, 1, "frame exhaustion must not validate a login");
  await f.noLeaks();
}));

test("all five production languages cover every label at 360px with usable touch confirmation", { timeout: 25_000 }, () => run({ viewport: { width: 360, height: 900 }, touch: true }, async f => {
  await open(f);
  for (const language of ["en", "zh-CN", "es", "ja", "ko"]) {
    await f.page.locator("#locale").selectOption(language);
    const result = await f.page.evaluate(async language => {
      const { labels } = await import("/platform/ui/labels.js");
      return { language: document.documentElement.lang,
        expectedKeys: Object.keys(labels.en).sort(), actualKeys: Object.keys(labels[language]).sort(),
        wrong: [...document.querySelectorAll("[data-i18n]")].filter(node =>
          typeof labels[language][node.dataset.i18n] !== "string"
          || !labels[language][node.dataset.i18n].trim()
          || node.textContent !== labels[language][node.dataset.i18n]).map(node => node.dataset.i18n),
        width: innerWidth, scroll: document.documentElement.scrollWidth };
    }, language);
    assert.equal(result.language, language); assert.deepEqual(result.actualKeys, result.expectedKeys);
    assert.deepEqual(result.wrong, []); assert.ok(result.scroll <= result.width + 1);
    await f.page.locator("#logout").scrollIntoViewIfNeeded(); await f.page.locator("#logout").tap();
    const dialog = f.page.locator("#logout-dialog");
    assert.equal(await dialog.evaluate(node => node.open), true);
    for (const id of ["cancel-logout", "confirm-logout"]) {
      const box = await f.page.locator(`#${id}`).boundingBox();
      assert.ok(box && box.width >= 44 && box.height >= 44 && box.x >= 0 && box.x + box.width <= 361);
      assert.equal(await f.page.locator(`#${id}`).evaluate(node => {
        const box = node.getBoundingClientRect();
        return node.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
      }), true);
    }
    await f.page.locator("#cancel-logout").tap();
  }
  assert.equal(f.apiRecords(LOGOUT).length, 0); assert.equal(f.apiRecords(SESSION).length, 1);
  await f.noLeaks();
}));

test("invalid credentials stay local while dispatched secrets clear before the mock server responds", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, 401, invalidSession);
} }, async f => {
  await open(f); await f.fill(); await f.page.locator("#username").fill("Teacher");
  await f.page.locator("#login").click(); await f.state("invalid-input");
  assert.equal(f.apiRecords(LOGIN).length, 0);
  await f.page.locator("#username").fill(username); await f.page.locator("#login").click();
  await waitFor(() => f.apiRecords(LOGIN).length === 1);
  assert.equal(await f.page.locator("#password").inputValue(), "");
  assert.equal(await f.page.locator("#otp").inputValue(), "");
  f.respond(f.apiRecords(LOGIN)[0], 503, '{"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}}');
  await f.state("unknown"); await idle(f); await f.noLeaks();
}));
