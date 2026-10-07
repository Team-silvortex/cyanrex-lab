import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

// Exercise the sanitizer actually embedded in generated Monaco assets, not only the npm package.
const require = createRequire(import.meta.url);
const { parse } = require("next/dist/compiled/acorn");
let browser, instrumentedChunk;

before(async () => {
  const entry = await readFile(new URL("../public/monaco/vs/editor/editor.main.js", import.meta.url), "utf8");
  const chunks = [...entry.matchAll(/"\.\.\/(editor\.api-[\w-]+)"/g)];
  assert.equal(chunks.length, 1, "one actual editor API dependency");
  const source = await readFile(new URL(`../public/monaco/vs/${chunks[0][1]}.js`, import.meta.url), "utf8");
  const declarations = [];
  const visit = node => {
    if (!node || typeof node !== "object") return;
    if (node.type === "VariableDeclaration" && node.declarations.some(item => item.id.name === "K_")) declarations.push(node);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(visit);
      else if (value && typeof value === "object") visit(value);
    }
  };
  visit(parse(source, { ecmaVersion: "latest" }));
  assert.equal(declarations.length, 1, "the pinned Monaco sanitizer instance must be unambiguous");
  const end = declarations[0].end;
  // Test-only instrumentation captures the same instance that Monaco's sanitize calls use.
  instrumentedChunk = `${source.slice(0, end)};globalThis.__sanitizerFixture = K_;${source.slice(end)}`;
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

async function run(callback) {
  const context = await browser.newContext();
  const errors = [], network = [];
  const page = await context.newPage();
  page.on("pageerror", error => errors.push(error.message));
  await context.route("**/*", route => { network.push(route.request().url()); return route.abort(); });
  try {
    await page.setContent('<main id="editor" style="width:800px;height:450px"></main>');
    await page.evaluate(() => {
      globalThis._VSCODE_NLS_MESSAGES = [];
      globalThis.__fixtureEvents = 0;
      globalThis.define = (id, dependencies, factory) => {
        if (!id.startsWith("vs/editor.api-") || JSON.stringify(dependencies) !== '["exports"]') throw new Error("Unexpected AMD fixture module");
        const exports = {};
        factory(exports);
        globalThis.__monacoFixture = exports.monaco;
      };
      globalThis.define.amd = {};
    });
    await page.addScriptTag({ content: instrumentedChunk });
    await callback(page);
    assert.deepEqual(errors, []);
    assert.deepEqual(network, [], "fixtures must not contact Engine, a CDN, or an attacker URL");
  } finally { await context.close(); }
}

test("generated Monaco uses the patched DOMPurify instance", () => run(async page => {
  const actual = await page.evaluate(() => ({ version: __sanitizerFixture.version, sanitize: typeof __sanitizerFixture.sanitize, editor: typeof __monacoFixture.editor.create }));
  assert.deepEqual(actual, { version: "3.4.16", sanitize: "function", editor: "function" });
}));

test("normal HTML sanitization preserves safe formatting and removes executable markup", () => run(async page => {
  const actual = await page.evaluate(() => {
    const root = document.createElement("div");
    root.innerHTML = __sanitizerFixture.sanitize('<b>safe fixture</b><script>globalThis.__fixtureEvents++</script><img onerror="globalThis.__fixtureEvents++"><a href="javascript:void(0)">bad link</a>');
    return { text: root.querySelector("b")?.textContent, scripts: root.querySelectorAll("script").length, handler: root.querySelector("img").getAttribute("onerror"), href: root.querySelector("a").getAttribute("href"), events: __fixtureEvents };
  });
  assert.deepEqual(actual, { text: "safe fixture", scripts: 0, handler: null, href: null, events: 0 });
}));

for (const [hook, custom] of [["beforeSanitizeElements", false], ["afterSanitizeElements", false], ["afterSanitizeAttributes", false], ["afterSanitizeElements", true]]) {
  test(`IN_PLACE ${hook} removal neutralizes detached ${custom ? "custom" : "ordinary"} descendants`, () => run(async page => {
    const actual = await page.evaluate(({ hook, custom }) => {
      const root = document.createElement("div");
      const wrapper = document.createElement(custom ? "fixture-wrapper" : "section");
      wrapper.id = "fixture-wrapper";
      const image = document.createElement("img");
      image.setAttribute("onerror", "globalThis.__fixtureEvents++");
      wrapper.append(image);
      root.append(wrapper);
      document.body.append(root);
      __sanitizerFixture.addHook(hook, node => { if (node === wrapper) node.remove(); });
      try {
        __sanitizerFixture.sanitize(root, { IN_PLACE: true, ...(custom ? { CUSTOM_ELEMENT_HANDLING: { tagNameCheck: /^fixture-wrapper$/ } } : {}) });
        image.dispatchEvent(new Event("error"));
        return { removed: wrapper.parentNode === null, handler: image.getAttribute("onerror"), events: __fixtureEvents };
      } finally { __sanitizerFixture.removeAllHooks(); root.remove(); }
    }, { hook, custom });
    assert.deepEqual(actual, { removed: true, handler: null, events: 0 });
  }));
}

test("IN_PLACE cannot return a force-removed raw-text root for unsafe reparsing", () => run(async page => {
  const actual = await page.evaluate(() => {
    const root = document.createElement("style");
    root.setAttribute("onclick", "globalThis.__fixtureEvents++");
    root.textContent = '</style><img onerror="globalThis.__fixtureEvents++">';
    document.body.append(root);
    try {
      const returned = __sanitizerFixture.sanitize(root, { IN_PLACE: true });
      return { rejected: false, returnedUnsafeRoot: returned === root };
    } catch (error) { return { rejected: error instanceof TypeError }; }
    finally { root.remove(); }
  });
  assert.deepEqual(actual, { rejected: true });
}));

test("healthy IN_PLACE roots retain identity and safe content", () => run(async page => {
  const actual = await page.evaluate(() => {
    const root = document.createElement("div");
    root.innerHTML = '<strong>keep fixture</strong><img onerror="globalThis.__fixtureEvents++">';
    document.body.append(root);
    try {
      const returned = __sanitizerFixture.sanitize(root, { IN_PLACE: true });
      return { same: returned === root, text: root.querySelector("strong")?.textContent, handler: root.querySelector("img").getAttribute("onerror") };
    } finally { root.remove(); }
  });
  assert.deepEqual(actual, { same: true, text: "keep fixture", handler: null });
}));

test("the real Monaco hover consumer still sanitizes and renders safe markdown", () => run(async page => {
  await page.evaluate(() => {
    const monaco = __monacoFixture;
    monaco.languages.register({ id: "sanitizer-fixture" });
    const provider = monaco.languages.registerHoverProvider("sanitizer-fixture", { provideHover: () => ({ contents: [{ value: '<strong>safe hover fixture</strong><img onerror="globalThis.__fixtureEvents++"><a href="javascript:void(0)">bad hover link</a>', supportHtml: true, isTrusted: false }] }) });
    const editor = monaco.editor.create(document.getElementById("editor"), { value: "fixture", language: "sanitizer-fixture", minimap: { enabled: false } });
    editor.setPosition({ lineNumber: 1, column: 2 });
    editor.trigger("fixture", "editor.action.showHover", {});
    globalThis.__disposeFixture = () => { editor.dispose(); provider.dispose(); };
  });
  try {
    await page.getByText("safe hover fixture", { exact: true }).waitFor({ state: "attached" });
    const actual = await page.locator(".monaco-hover").evaluate(root => ({ handlers: root.querySelectorAll("[onerror],script").length, badLinks: [...root.querySelectorAll("a")].filter(link => link.getAttribute("href")?.startsWith("javascript:")).length, events: globalThis.__fixtureEvents }));
    assert.deepEqual(actual, { handlers: 0, badLinks: 0, events: 0 });
  } finally { await page.evaluate(() => __disposeFixture()); }
}));

test("the unmodified self-hosted AMD loader resolves the new editor chunk and language graph", { timeout: 30_000 }, async () => {
  const context = await browser.newContext(), page = await context.newPage();
  const origin = "https://monaco-fixture.invalid", errors = [], scripts = [];
  const publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  page.on("pageerror", error => errors.push(error.message));
  await context.route("**/*", async route => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { errors.push("Unexpected external request"); return route.abort(); }
    if (url.pathname === "/") return route.fulfill({ contentType: "text/html", body: '<div id="editor" style="width:800px;height:450px"></div>' });
    const file = path.resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
    const assets = path.join(publicRoot, "monaco", "vs") + path.sep;
    if (!file.startsWith(assets)) { errors.push("Unexpected fixture path"); return route.abort(); }
    try {
      const body = await readFile(file);
      if (file.endsWith(".js")) scripts.push(url.pathname);
      const contentType = file.endsWith(".js") ? "application/javascript" : file.endsWith(".css") ? "text/css" : "application/octet-stream";
      return route.fulfill({ body, contentType });
    } catch { errors.push("Missing generated fixture asset"); return route.fulfill({ status: 404 }); }
  });
  try {
    await page.goto(origin);
    await page.addScriptTag({ url: `${origin}/monaco/vs/loader.js` });
    const actual = await page.evaluate(async () => {
      require.config({ paths: { vs: "/monaco/vs" } });
      await new Promise((resolve, reject) => require(["vs/editor/editor.main"], resolve, reject));
      const editor = monaco.editor.create(document.getElementById("editor"), { value: "keep fixture", language: "plaintext", minimap: { enabled: false } });
      try { return { text: editor.getValue(), languages: monaco.languages.getLanguages().map(language => language.id) }; }
      finally { editor.dispose(); }
    });
    assert.equal(actual.text, "keep fixture");
    for (const language of ["typescript", "javascript", "json", "css", "html", "python"]) assert.ok(actual.languages.includes(language), language);
    assert.ok(scripts.some(file => /editor\.api-cyanrex-[a-f0-9]{16}\.js$/.test(file)));
    assert.ok(scripts.every(file => !file.includes("editor.api-CalNCsUg")), "no request for the old sanitizer URL");
    assert.deepEqual(errors, []);
  } finally { await context.close(); }
});
