import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { baseUrl, setupTask, taskDraft, textItem, confirmation, approve, waitForErrors } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

function taskTest(name, body, options) {
  test(name, { timeout: 60000 }, async () => {
    const fixture = await setupTask(browser, options);
    try { await body(fixture); await fixture.assertLocalOnly(); }
    finally { await fixture.close(); }
  });
}

async function keptDraft(f) {
  await f.page.getByTestId("task-title").fill("Keep this private task");
  await f.add();
  await f.page.getByTestId("editor-filename").fill("keep.txt");
  await f.setSource("Private payload must remain unchanged.");
  return f.export();
}

taskTest("a new task accepts a non-code title without inventing any payload or editor document", async f => {
  assert.deepEqual(await f.ids(), []);
  assert.equal(await f.page.locator(".monaco-editor").count(), 0);
  assert.equal(await f.page.getByTestId("task-title").inputValue(), "");
  assert.equal(await f.page.locator('nav a[href="/tasks/new"][aria-current="page"]').count(), 1);
  await f.page.getByTestId("task-title").fill("Discuss next week's reading");
  assert.deepEqual(await f.export(), taskDraft([], "Discuss next week's reading"));
  assert.ok((await f.page.getByTestId("task-draft-status").textContent()).trim());
});

taskTest("the editor compatibility route renders the same empty task container and adds plain text explicitly", async f => {
  assert.deepEqual(await f.ids(), []);
  const id = await f.add();
  await f.current({ filename: "untitled.txt", language: "plaintext", text: "" });
  assert.deepEqual((await f.export()).payload, [textItem(id)]);
}, { path: "/editor" });

taskTest("multiple payloads preserve their independent language, filename and text across repeated selection", async f => {
  await f.page.getByTestId("task-title").fill("Writing and implementation");
  const expected = [];
  for (const value of [
    { filename: "plan.md", language: "markdown", text: "# Plan\n\nDiscuss a document, not a lab." },
    { filename: "lib.rs", language: "rust", text: "fn main() { println!(\"local only\"); }" },
    { filename: "settings.json", language: "json", text: '{"enabled":true}' },
  ]) {
    const id = await f.add();
    await f.current({ filename: "untitled.txt", language: "plaintext", text: "" });
    await f.language(value.language);
    await f.page.getByTestId("editor-filename").fill(value.filename);
    await f.setSource(value.text);
    expected.push({ id, ...value });
  }
  for (const value of [...expected, ...expected].reverse()) {
    await f.item(value.id).click();
    await f.current(value);
  }
  const exported = await f.export();
  assert.equal(exported.title, "Writing and implementation");
  assert.deepEqual(exported.payload.map(({ id, filename, language, text }) => ({ id, filename, language, text })), expected);
  assert.ok(exported.payload.every(item => item.kind === "text" && Number.isSafeInteger(item.revision) && item.revision >= 1));
});

taskTest("payload removal names the selected target, cancels safely and removes only that item after confirmation", async f => {
  const first = await f.add();
  await f.page.getByTestId("editor-filename").fill("keep-first.txt");
  await f.setSource("First payload stays.");
  const second = await f.add();
  await f.page.getByTestId("editor-filename").fill("remove-second.txt");
  await f.setSource("Second payload is the selected removal target.");
  const before = await f.export();
  await f.page.getByTestId("task-payload-remove").click();
  await confirmation(f.page).getByText("remove-second.txt", { exact: true }).waitFor();
  assert.deepEqual(await f.ids(), [first, second]);
  await confirmation(f.page).getByRole("button", { name: "取消", exact: true }).click();
  assert.deepEqual(await f.export(), before);
  await f.page.getByTestId("task-payload-remove").click();
  await confirmation(f.page).getByText("remove-second.txt", { exact: true }).waitFor();
  await approve(f.page).click();
  await confirmation(f.page).waitFor({ state: "detached" });
  assert.deepEqual(await f.ids(), [first]);
  assert.deepEqual((await f.export()).payload, [before.payload[0]]);
});

taskTest("whole-task reset preserves everything on cancel and clears title and all payloads only after approval", async f => {
  const before = await keptDraft(f);
  await f.page.getByTestId("task-draft-reset").click();
  await confirmation(f.page).waitFor();
  await f.page.keyboard.press("Escape");
  assert.deepEqual(await f.export(), before);
  await f.page.getByTestId("task-draft-reset").click();
  await approve(f.page).click();
  await confirmation(f.page).waitFor({ state: "detached" });
  assert.deepEqual(await f.export(), taskDraft());
  assert.equal(await f.page.locator(".monaco-editor").count(), 0);
  assert.equal(await f.page.evaluate(() => window.monaco?.editor.getModels().length || 0), 0);
});

taskTest("whole-task JSON round-trips exact identities and local revisions without runtime or owner fields", async f => {
  const before = await keptDraft(f);
  const imported = taskDraft([
    textItem("notes_v1", { revision: 7, filename: "notes.md", language: "markdown", text: "# 私人计划\n\n保留引号：\"hello\"。\n" }),
    textItem("draft_code", { revision: 4, filename: "draft.ts", language: "typescript", text: "const count: number = 7;\n" }),
  ], "Mixed text task");
  await f.chooseImport(imported);
  await confirmation(f.page).waitFor();
  await confirmation(f.page).getByRole("button", { name: "取消", exact: true }).click();
  assert.deepEqual(await f.export(), before);
  await f.import(imported);
  assert.deepEqual(await f.ids(), imported.payload.map(item => item.id));
  for (const item of imported.payload) { await f.item(item.id).click(); await f.current(item); }
  assert.deepEqual(await f.export(), imported);
  const stored = await f.page.evaluate(() => [...Object.values(localStorage), ...Object.values(sessionStorage)]);
  assert.ok(stored.every(value => !value.includes("Mixed text task") && !value.includes("私人计划")));
});

taskTest("rejected live text edits restore accepted content without consuming a payload revision", async f => {
  const before = await keptDraft(f), accepted = before.payload[0];
  for (const rejected of ["x".repeat(256 * 1024 + 1), "injected\0control", "injected\u0085control"]) {
    // The setter represents editor input, not a shortcut to the parent's draft state.
    await f.page.evaluate(text => window.monaco.editor.getEditors()[0].setValue(text), rejected);
    await f.page.waitForFunction(text => window.monaco.editor.getModels()[0]?.getValue() === text, accepted.text);
    await f.page.getByRole("alert").first().waitFor();
    assert.deepEqual(await f.export(), before);
  }
  await f.setSource("A subsequent valid edit is accepted once.");
  const next = await f.export();
  assert.equal(next.payload[0].text, "A subsequent valid edit is accepted once.");
  assert.equal(next.payload[0].revision, accepted.revision + 1);
  assert.equal(next.payload[0].id, accepted.id);
});

taskTest("whole-task replacement with identical payload identities and revisions disposes the old model", async f => {
  const first = taskDraft([textItem("same_identity", {
    revision: 9, filename: "same.ts", language: "typescript", text: 'const count: number = "old error";',
  })], "Old draft");
  await f.import(first);
  await f.current(first.payload[0]);
  await waitForErrors(f.page, true);
  const old = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
  try {
    const replacement = taskDraft([{ ...first.payload[0], text: "const count: number = 7;" }], "Replacement draft");
    await f.import(replacement);
    await f.current(replacement.payload[0]);
    const previous = await old.evaluate(model => ({ disposed: model.isDisposed(), uri: model.uri.toString() }));
    assert.equal(previous.disposed, true);
    const current = await f.page.evaluate(async () => {
      const monaco = window.monaco, model = monaco.editor.getModels()[0];
      const worker = await (await monaco.typescript.getTypeScriptWorker())(model.uri);
      return { uri: model.uri.toString(), diagnostics: await worker.getSemanticDiagnostics(model.uri.toString()) };
    });
    assert.notEqual(current.uri, previous.uri);
    assert.deepEqual(current.diagnostics, []);
    await waitForErrors(f.page, false);
    assert.deepEqual(await f.export(), replacement);
    await f.setSource("const count: number = 8;");
    assert.deepEqual(await f.export(), taskDraft([{ ...replacement.payload[0], revision: 10, text: "const count: number = 8;" }], replacement.title));
  } finally { await old.dispose(); }
});

taskTest("a scripted edit during child file confirmation is rejected and cannot silently enter the task", async f => {
  const before = await keptDraft(f);
  await f.page.getByTestId("editor-import").setInputFiles({
    name: "candidate.txt", mimeType: "text/plain", buffer: Buffer.from("Explicitly reviewed replacement."),
  });
  await confirmation(f.page).waitFor();
  assert.equal(await f.page.evaluate(() => {
    const monaco = window.monaco;
    return monaco.editor.getEditors()[0].getOption(monaco.editor.EditorOption.readOnly);
  }), true);
  // Normal typing is read-only; probe the parent guard even if a script bypasses that UI option.
  await f.page.evaluate(() => window.monaco.editor.getEditors()[0].setValue("Unreviewed scripted replacement."));
  await f.page.waitForFunction(text => window.monaco.editor.getModels()[0]?.getValue() === text, before.payload[0].text);
  const notice = f.page.locator(".language-workspace .editor-notice[role=alert]");
  await notice.waitFor();
  assert.ok((await notice.textContent()).trim());
  assert.doesNotMatch(await notice.textContent(), /taskDraft\./);
  await confirmation(f.page).getByRole("button", { name: "取消", exact: true }).click();
  assert.deepEqual(await f.export(), before);
  await f.current(before.payload[0]);
});

taskTest("a delayed child file read cannot update a new task after history navigation disposes its owner", async f => {
  // Give browser Back a real same-document Next route, without using application internals.
  await f.page.locator('nav a[href="/account"]').click();
  await f.page.waitForURL(`${baseUrl}/account`);
  await f.page.locator('nav a[href="/tasks/new"]').click();
  await f.page.waitForURL(`${baseUrl}/tasks/new`);
  await keptDraft(f);
  const held = await f.page.evaluateHandle(() => {
    const descriptor = Object.getOwnPropertyDescriptor(File.prototype, "arrayBuffer");
    const original = File.prototype.arrayBuffer;
    const state = {
      started: false, release: null,
      restore() {
        if (descriptor) Object.defineProperty(File.prototype, "arrayBuffer", descriptor);
        else delete File.prototype.arrayBuffer;
      },
    };
    Object.defineProperty(File.prototype, "arrayBuffer", { configurable: true, writable: true, value: function () {
      if (this.name !== "delayed-child.txt") return original.call(this);
      state.started = true;
      return new Promise((resolve, reject) => {
        state.release = async () => {
          try { resolve(await original.call(this)); } catch (error) { reject(error); }
        };
      });
    } });
    return state;
  });
  try {
    await f.page.getByTestId("editor-import").setInputFiles({
      name: "delayed-child.txt", mimeType: "text/plain", buffer: Buffer.from("Late old-owner bytes must not publish."),
    });
    await confirmation(f.page).waitFor();
    await approve(f.page).click();
    await f.page.waitForFunction(state => state.started, held);
    await f.page.goBack();
    await f.page.waitForURL(`${baseUrl}/account`);
    await f.page.waitForFunction(() => window.monaco.editor.getModels().length === 0);
    await f.page.locator('nav a[href="/tasks/new"]').click();
    await f.page.waitForURL(`${baseUrl}/tasks/new`);
    await f.page.getByTestId("task-title").fill("A newly owned task");
    await f.add();
    await f.setSource("New task content must survive a late old read.");
    const before = await f.export();
    await held.evaluate(async state => { await state.release(); state.restore(); });
    assert.deepEqual(await f.export(), before);
    await f.current(before.payload[0]);
  } finally {
    await held.evaluate(state => state.restore());
    await held.dispose();
  }
});

for (const [label, invalid] of [
  ["unsupported payload kind", taskDraft([textItem("unknown", { kind: "code" })], "Rejected kind")],
  ["duplicate payload identities", taskDraft([textItem("duplicate"), textItem("duplicate", { text: "different" })])],
  ["invalid UTF-8 bytes", Buffer.from([0x7b, 0x22, 0xff, 0x22, 0x7d])],
]) {
  taskTest(`task import rejects ${label} and leaves the entire previous draft unchanged`, async f => {
    const before = await keptDraft(f);
    await f.rejectImport(invalid);
    assert.deepEqual(await f.export(), before);
    await f.current(before.payload[0]);
  });
}

taskTest("task import enforces item, text and JSON byte bounds and rejects additional authority fields", async f => {
  const before = await keptDraft(f);
  for (const invalid of [
    taskDraft(Array.from({ length: 33 }, (_, index) => textItem(`item_${index}`))),
    taskDraft([textItem("oversized", { text: "你".repeat(87382) })]),
    { ...taskDraft(), owner: "not-client-authority" },
    taskDraft([textItem("unknown_language", { language: "not-a-language" })]),
  ]) {
    await f.rejectImport(invalid);
    assert.deepEqual(await f.export(), before);
  }
  await f.chooseImport(Buffer.alloc(8 * 1024 * 1024 + 1, 0x20));
  await f.page.locator(".task-draft-notice[role='alert']").waitFor();
  assert.equal(await confirmation(f.page).count(), 0);
  assert.deepEqual(await f.export(), before);
});

taskTest("switching payloads disposes old models and prevents stale diagnostics from contaminating another document", async f => {
  const bad = await f.add();
  await f.language("typescript");
  await f.setSource('const count: number = "wrong";');
  await waitForErrors(f.page, true);
  const badFilename = await f.page.getByTestId("editor-filename").inputValue();
  const oldModel = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
  try {
    const good = await f.add();
    await f.language("json");
    await f.setSource('{"safe":true}');
    assert.equal(await oldModel.evaluate(model => model.isDisposed()), true);
    for (let index = 0; index < 2; index++) {
      await f.item(bad).click();
      await f.current({ filename: badFilename, language: "typescript", text: 'const count: number = "wrong";' });
      await waitForErrors(f.page, true);
      await f.item(good).click();
      await f.page.waitForFunction(() => window.monaco.editor.getModels()[0]?.getLanguageId() === "json");
      const diagnostics = await f.page.evaluate(async () => {
        const monaco = window.monaco, model = monaco.editor.getModels()[0];
        const worker = await (await monaco.json.getWorker())(model.uri);
        const validated = await worker.doValidation(model.uri.toString());
        return { uri: model.uri.toString(), validated, markers: monaco.editor.getModelMarkers({}).map(marker => ({
          uri: marker.resource.toString(), code: typeof marker.code === "object" ? marker.code.value : marker.code,
        })) };
      });
      assert.deepEqual(diagnostics.validated, []);
      assert.ok(diagnostics.markers.every(marker => marker.uri === diagnostics.uri && String(marker.code) !== "2322"));
      assert.equal(await f.source(), '{"safe":true}');
      assert.equal(await f.page.evaluate(() => window.monaco.editor.getModels().length), 1);
    }
    f.page.once("dialog", dialog => { void dialog.accept(); });
    await f.page.locator('nav a[href="/account"]').click();
    await f.page.waitForURL(`${baseUrl}/account`);
    await f.page.waitForFunction(() => window.monaco.editor.getModels().length === 0);
  } finally { await oldModel.dispose(); }
});

taskTest("task and payload controls fit narrow screens in all four interface languages without raw translation keys", async f => {
  await f.page.getByTestId("task-title").fill("A writing task with independently editable content");
  await f.add();
  await f.page.getByTestId("editor-filename").fill("a-long-but-valid-payload-filename.md");
  await f.setSource("A private writing payload, not a program.");
  for (const locale of ["zh-CN", "en", "es", "ja"]) {
    await f.page.locator(".menu-toggle").click();
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
    await f.page.keyboard.press("Escape");
    assert.equal(await f.page.locator(".menu-toggle").getAttribute("aria-expanded"), "false");
    await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
    assert.doesNotMatch(await f.page.locator("main").innerText(), /\b(?:tasks?|taskDraft|editor|layout)\.[A-Za-z][\w.]*/);
    for (const id of ["task-title", "task-payload-add", "task-draft-export", "editor-filename"]) {
      const box = await f.page.getByTestId(id).boundingBox();
      assert.ok(box.x >= 0 && box.x + box.width <= 320, `${locale}: ${id} must fit the viewport`);
    }
    assert.equal(await f.source(), "A private writing payload, not a program.");
  }
}, { viewport: { width: 320, height: 844 } });
