import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { baseUrl, setupEditor, confirmation, approve, waitForMarkers } from "./helpers/multiLanguageEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

function editorTest(name, body, options) {
  test(name, { timeout: 45000 }, async () => {
    const fixture = await setupEditor(browser, options);
    try { await body(fixture); await fixture.assertNoLeaks(); }
    finally { await fixture.close(); }
  });
}

editorTest("a student can explicitly add TypeScript content to a task with local language assets", async f => {
  assert.equal(await f.page.getByTestId("language-select").inputValue(), "typescript");
  assert.equal(await f.source(), "");
  assert.equal(await f.page.locator('nav a[href="/tasks/new"][aria-current="page"]').count(), 1);
  for (const path of ["/settings", "/terminal", "/teaching", "/modules"]) {
    assert.equal(await f.page.locator(`nav a[href="${path}"]`).count(), 0);
  }
  const options = await f.page.getByTestId("language-select").locator("option").evaluateAll(items => items.map(item => item.value));
  for (const language of ["typescript", "javascript", "json", "html", "css", "rust", "python", "c", "cpp", "markdown", "yaml", "sql", "shell", "plaintext"]) {
    assert.ok(options.includes(language), `${language} must be an explicit available language`);
  }
  const capability = await f.page.getByTestId("editor-capabilities").textContent();
  assert.ok(capability.trim());
  assert.doesNotMatch(capability, /editor\.[A-Za-z]/);
});

editorTest("real JSON diagnostics clear after correction and the format button changes actual document text", async f => {
  await f.selectLanguage("json");
  await f.setSource('{"enabled": }');
  await waitForMarkers(f.page, true);
  assert.ok((await f.page.getByTestId("editor-problems").textContent()).trim());
  const value = { enabled: true, items: [1, 2] };
  await f.setSource(JSON.stringify(value));
  await waitForMarkers(f.page, false);
  assert.equal(await f.page.getByTestId("editor-format").isEnabled(), true);
  await f.page.getByTestId("editor-format").click();
  await f.page.waitForFunction(() => window.monaco.editor.getModels()[0].getValue().includes("\n"));
  assert.deepEqual(JSON.parse(await f.source()), value);
  assert.ok(f.assetRequests.some(path => /json(?:Mode|\.worker)/.test(path)), "JSON uses the shipped language service");
});

editorTest("real JSON validation never fetches a document-selected external schema", async f => {
  await f.selectLanguage("json");
  const source = '{"$schema":"https://editor-schema.invalid/private.json","enabled":true}';
  await f.setSource(source);
  const result = await f.page.evaluate(async () => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0];
    const worker = await (await monaco.json.getWorker())(model.uri);
    return {
      diagnostics: await worker.doValidation(model.uri.toString()),
      requestsEnabled: monaco.json.jsonDefaults.diagnosticsOptions.enableSchemaRequest,
    };
  });
  assert.equal(result.requestsEnabled, false);
  assert.ok(Array.isArray(result.diagnostics), "the actual JSON worker completed validation");
  assert.equal(f.networkRequests.filter(request => request.origin === "https://editor-schema.invalid").length, 0);
  assert.equal(await f.source(), source);
});

editorTest("real TypeScript worker reports types and returns completion, hover and exact definition locations", async f => {
  await f.setSource('const count: number = "wrong";');
  await waitForMarkers(f.page, true);
  const code = await f.page.evaluate(async () => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0];
    const worker = await (await monaco.typescript.getTypeScriptWorker())(model.uri);
    return (await worker.getSemanticDiagnostics(model.uri.toString())).map(item => item.code);
  });
  assert.ok(code.includes(2322), "the real TypeScript service must reject a string assigned to number");
  const source = 'const greeting = "hello";\ngreeting.';
  await f.setSource(source);
  const result = await f.page.evaluate(async () => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0], text = model.getValue();
    const worker = await (await monaco.typescript.getTypeScriptWorker())(model.uri);
    const uri = model.uri.toString(), use = text.lastIndexOf("greeting") + 2;
    const [completion, hover, definitions] = await Promise.all([
      worker.getCompletionsAtPosition(uri, text.length),
      worker.getQuickInfoAtPosition(uri, use),
      worker.getDefinitionAtPosition(uri, use),
    ]);
    return {
      uri, names: completion?.entries.map(item => item.name),
      hover: hover?.displayParts.map(part => part.text).join(""),
      definitions: definitions?.map(item => ({ fileName: item.fileName, start: item.textSpan.start })),
    };
  });
  assert.ok(result.names?.includes("toUpperCase"));
  assert.match(result.hover, /greeting/);
  assert.ok(result.definitions?.some(item => item.fileName === result.uri && item.start === source.indexOf("greeting")));
  assert.ok(f.assetRequests.some(path => /ts(?:Mode|\.worker)/.test(path)), "TypeScript uses the shipped worker");
});

editorTest("real JavaScript checkJs diagnostics reject a JSDoc type mismatch and clear after correction", async f => {
  await f.selectLanguage("javascript");
  await f.setSource('/** @type {number} */\nconst count = "wrong";');
  await waitForMarkers(f.page, true);
  const codes = await f.page.evaluate(async () => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0];
    const worker = await (await monaco.typescript.getJavaScriptWorker())(model.uri);
    return (await worker.getSemanticDiagnostics(model.uri.toString())).map(item => item.code);
  });
  assert.ok(codes.includes(2322), "checkJs must perform semantic checks, not only JavaScript highlighting");
  await f.setSource("/** @type {number} */\nconst count = 1;");
  await waitForMarkers(f.page, false);
});

editorTest("real TypeScript modules do not accidentally share globals when both documents reach the worker", async f => {
  await f.setSource("editorFixtureForeignOnly;\neditorFixtureF");
  const result = await f.page.evaluate(async () => {
    const monaco = window.monaco, own = monaco.editor.getModels()[0];
    const foreign = monaco.editor.createModel(
      'const editorFixtureForeignOnly = 7;\nconst wrong: number = "mismatch";',
      "typescript", monaco.Uri.parse("inmemory://editor-browser-fixture/other-document.ts"),
    );
    try {
      // Explicitly synchronize both real models: disabled eager sync alone cannot satisfy this test.
      const worker = await (await monaco.typescript.getTypeScriptWorker())(own.uri, foreign.uri);
      const [ownDiagnostics, foreignDiagnostics, completions, definitions] = await Promise.all([
        worker.getSemanticDiagnostics(own.uri.toString()),
        worker.getSemanticDiagnostics(foreign.uri.toString()),
        worker.getCompletionsAtPosition(own.uri.toString(), own.getValue().length),
        worker.getDefinitionAtPosition(own.uri.toString(), 5),
      ]);
      return {
        ownDiagnostics: ownDiagnostics.map(item => ({ code: item.code, message: item.messageText })),
        foreignCodes: foreignDiagnostics.map(item => item.code),
        names: completions?.entries.map(item => item.name) || [],
        definitions: definitions || [],
      };
    } finally { foreign.dispose(); }
  });
  assert.ok(result.foreignCodes.includes(2322), "the second document was genuinely present in the worker");
  assert.ok(result.ownDiagnostics.some(item => item.code === 2304 && String(item.message).includes("editorFixtureForeignOnly")));
  assert.equal(result.names.includes("editorFixtureForeignOnly"), false);
  assert.deepEqual(result.definitions, []);
  assert.equal(await f.page.evaluate(() => window.monaco.editor.getModels().length), 1);
  // This checks module semantics, not security isolation between scripts in the same browser realm.
});

editorTest("real CSS diagnostics identify malformed declarations and formatting preserves corrected rules", async f => {
  await f.selectLanguage("css");
  await f.setSource("body { color red; }");
  await waitForMarkers(f.page, true);
  await f.setSource("body{color:red;margin:0}");
  await waitForMarkers(f.page, false);
  await f.page.getByTestId("editor-format").click();
  await f.page.waitForFunction(() => window.monaco.editor.getModels()[0].getValue().includes("\n"));
  const formatted = await f.source();
  assert.match(formatted, /color:\s+red/);
  assert.match(formatted, /margin:\s*0/);
  assert.ok(f.assetRequests.some(path => /css(?:Mode|\.worker)/.test(path)), "CSS uses its shipped language service");
});

editorTest("real HTML formatting preserves markup while its profile explicitly offers no diagnostics", async f => {
  await f.selectLanguage("html");
  await f.setSource("<html><head><title>Fixture</title></head><body><div><span>Hello</span></div></body></html>");
  assert.doesNotMatch(await f.page.getByTestId("editor-capabilities").textContent(), /诊断/);
  assert.match(await f.page.getByTestId("editor-problems").textContent(), /尚未接入诊断服务/);
  await f.page.getByTestId("editor-format").click();
  await f.page.waitForFunction(() => window.monaco.editor.getModels()[0].getValue().includes("\n"));
  const document = await f.page.evaluate(() => {
    const parsed = new DOMParser().parseFromString(window.monaco.editor.getModels()[0].getValue(), "text/html");
    return { title: parsed.title, text: parsed.querySelector("body > div > span")?.textContent };
  });
  assert.deepEqual(document, { title: "Fixture", text: "Hello" });
  assert.ok(f.assetRequests.some(path => /html(?:Mode|\.worker)/.test(path)), "HTML uses its shipped language service");
});

editorTest("Rust exposes a real basic snippet without claiming semantic formatting support", async f => {
  const semanticCapabilities = await f.page.getByTestId("editor-capabilities").textContent();
  await f.selectLanguage("rust");
  assert.notEqual(await f.page.getByTestId("editor-capabilities").textContent(), semanticCapabilities);
  assert.equal(await f.page.getByTestId("editor-format").isDisabled(), true);
  await f.setSource("fn");
  await f.page.getByTestId("editor-completion").click();
  const suggestion = f.page.locator(".suggest-widget .monaco-list-row").filter({ hasText: "Rust function snippet" });
  await suggestion.waitFor();
  await suggestion.click();
  await f.page.waitForFunction(() => /fn\s+name\([^)]*\)\s*\{/.test(window.monaco.editor.getModels()[0].getValue()));
  assert.match(await f.source(), /\n/);
});

editorTest("changing languages or filename preserves the single document and never installs eBPF behavior", async f => {
  const source = "// private editor fixture\nconst unchanged = 17;";
  await f.setSource(source);
  for (const language of ["python", "c", "cpp", "markdown", "json", "typescript"]) {
    await f.selectLanguage(language);
    assert.equal(await f.source(), source);
    assert.equal(await f.page.evaluate(() => window.monaco.editor.getModels().length), 1);
  }
  await f.page.getByTestId("editor-filename").fill("local-draft.ts");
  assert.equal(await f.source(), source);
  const stored = await f.page.evaluate(() => [...Object.values(localStorage), ...Object.values(sessionStorage)]);
  assert.ok(stored.every(value => !value.includes("private editor fixture")), "private text is not silently persisted in browser storage");
});

editorTest("file replacement requires confirmation and cancelling preserves text, filename and language", async f => {
  const original = "const untouched = 41;", imported = "# 私人文件\n\nImported only into browser memory.\n";
  await f.setSource(original);
  await f.page.getByTestId("editor-filename").fill("keep.ts");
  const file = { name: "notes.md", mimeType: "text/markdown", buffer: Buffer.from(imported) };
  await f.page.getByTestId("editor-import").setInputFiles(file);
  await confirmation(f.page).waitFor();
  assert.equal(await f.source(), original);
  await confirmation(f.page).getByRole("button", { name: "取消", exact: true }).click();
  await confirmation(f.page).waitFor({ state: "detached" });
  assert.equal(await f.source(), original);
  assert.equal(await f.page.getByTestId("editor-filename").inputValue(), "keep.ts");
  assert.equal(await f.page.getByTestId("language-select").inputValue(), "typescript");
  await f.page.getByTestId("editor-import").setInputFiles(file);
  await confirmation(f.page).waitFor();
  await approve(f.page).click();
  await confirmation(f.page).waitFor({ state: "detached" });
  await f.page.waitForFunction(text => window.monaco.editor.getModels()[0]?.getValue() === text, imported);
  assert.equal(await f.page.getByTestId("editor-filename").inputValue(), "notes.md");
});

for (const [name, buffer] of [
  ["NUL bytes", Buffer.from("hello\0private")],
  ["invalid UTF-8", Buffer.from([0x66, 0x80, 0xff])],
  ["a byte beyond 256 KiB", Buffer.alloc(256 * 1024 + 1, 0x61)],
  ["multibyte text exceeding the byte limit", Buffer.from("你".repeat(87382))],
]) {
  editorTest(`import rejects ${name} without replacing or uploading the current document`, async f => {
    const source = "const preserved = 1;";
    await f.setSource(source);
    await f.page.getByTestId("editor-filename").fill("preserved.ts");
    await f.page.getByTestId("editor-import").setInputFiles({ name: "invalid.txt", mimeType: "text/plain", buffer });
    if (buffer.length <= 256 * 1024) {
      await confirmation(f.page).waitFor();
      await approve(f.page).click();
    }
    await f.page.locator(".language-workspace .editor-notice[role='alert'], .safety-dialog [role='alert']").waitFor();
    if (buffer.length <= 256 * 1024) {
      await confirmation(f.page).getByRole("button", { name: "关闭", exact: true }).click();
      await confirmation(f.page).waitFor({ state: "detached" });
    } else assert.equal(await confirmation(f.page).count(), 0);
    assert.equal(await f.source(), source);
    assert.equal(await f.page.getByTestId("editor-filename").inputValue(), "preserved.ts");
    assert.equal(await f.page.getByTestId("language-select").inputValue(), "typescript");
  });
}

editorTest("download exports the exact UTF-8 text locally and reset remains an explicit confirmed replacement", async f => {
  const source = "// 本地导出\nconst answer = 42;\n";
  await f.setSource(source);
  await f.page.getByTestId("editor-filename").fill("local-only.ts");
  const pending = f.page.waitForEvent("download");
  await f.page.getByTestId("editor-download").click();
  const download = await pending;
  assert.equal(download.suggestedFilename(), "local-only.ts");
  const chunks = [];
  for await (const chunk of await download.createReadStream()) chunks.push(chunk);
  assert.equal(Buffer.concat(chunks).toString("utf8"), source);
  assert.equal(await f.source(), source);
  await f.page.getByTestId("editor-reset").click();
  await confirmation(f.page).waitFor();
  await f.page.keyboard.press("Escape");
  assert.equal(await f.source(), source);
  await f.page.getByTestId("editor-reset").click();
  await approve(f.page).click();
  await confirmation(f.page).waitFor({ state: "detached" });
  await f.page.waitForFunction(() => window.monaco.editor.getModels()[0]?.getValue() === "");
});

editorTest("client-side navigation disposes the old Monaco model and returning starts an empty task", async f => {
  await f.setSource("const mustNotSurviveUnmount = true;");
  await f.page.evaluate(() => { window.editorFixtureOldModel = window.monaco.editor.getModels()[0]; });
  const leaving = f.page.waitForEvent("dialog");
  f.page.once("dialog", dialog => { void dialog.accept(); });
  await f.page.locator('nav a[href="/account"]').click();
  assert.equal((await leaving).type(), "confirm");
  await f.page.waitForURL(`${baseUrl}/account`);
  await f.page.waitForFunction(() => window.editorFixtureOldModel.isDisposed() && window.monaco.editor.getModels().length === 0);
  await f.page.locator('nav a[href="/tasks/new"]').click();
  await f.page.waitForURL(`${baseUrl}/tasks/new`);
  await f.page.getByTestId("task-payload-add").waitFor();
  assert.equal(await f.page.locator(".monaco-editor").count(), 0);
  assert.equal(await f.page.locator("[data-payload-id]").count(), 0);
  await f.page.getByTestId("task-payload-add").click();
  await f.page.locator(".monaco-editor").waitFor();
  await f.page.waitForFunction(() => window.monaco.editor.getModels().length === 1);
  assert.equal(await f.source(), "");
  assert.equal(await f.page.evaluate(() => window.monaco.editor.getModels()[0] === window.editorFixtureOldModel), false);
});

editorTest("all four interface languages fit a narrow editor without exposing untranslated keys", async f => {
  for (const locale of ["zh-CN", "en", "es", "ja"]) {
    await f.page.locator(".menu-toggle").click();
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    assert.ok((await f.page.locator(".task-draft-workspace .page-header h2").textContent()).trim());
    await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
    assert.doesNotMatch(await f.page.locator(".sidebar").innerText(), /\blayout\.[A-Za-z][\w.]*/);
    await f.page.keyboard.press("Escape");
    assert.equal(await f.page.locator(".menu-toggle").getAttribute("aria-expanded"), "false");
    await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
    assert.doesNotMatch(await f.page.locator(".language-workspace").innerText(), /\beditor\.[A-Za-z][\w.]*/);
    const button = await f.page.getByTestId("editor-download").boundingBox();
    assert.ok(button.x >= 0 && button.x + button.width <= 320, `${locale} download action must fit the viewport`);
  }
}, { viewport: { width: 320, height: 844 } });
