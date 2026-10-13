import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import {
  baseUrl, engineUrl, setupTask, taskDraft, textItem, confirmation, approve,
} from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

function taskTextTest(name, body) {
  test(name, { timeout: 90000 }, async () => {
    const f = await setupTask(browser);
    let failure;
    try { await body(f); } catch (error) { failure = error; }
    finally {
      try {
        try { await f.assertLocalOnly(); }
        catch (error) { failure = failure ? new AggregateError([failure, error], "Behavior and isolation failed") : error; }
      }
      finally { await f.close(); }
    }
    if (failure) throw failure;
  });
}

const file = (name, text, mimeType = "text/plain") => ({
  name, mimeType, buffer: Buffer.isBuffer(text) ? text : Buffer.from(text, "utf8"),
});
async function chooseFiles(f, files) {
  await f.page.getByTestId("task-text-import-open").waitFor();
  const input = f.page.getByTestId("task-text-import");
  assert.equal(await input.evaluate(element => element.multiple), true);
  await input.setInputFiles(files);
}
async function confirmedFiles(f, files) {
  await chooseFiles(f, files);
  await confirmation(f.page).waitFor();
  await approve(f.page).click();
  await confirmation(f.page).waitFor({ state: "detached" });
}
async function seed(f) {
  const draft = taskDraft([textItem("existing_private_item", {
    revision: 7, filename: "kept.txt", language: "plaintext", text: "Keep the original payload.\n",
  })], "Existing private task title");
  await f.import(draft);
  await f.current(draft.payload[0]);
  return draft;
}

// Only delay actual native File reads; the original method supplies every accepted byte.
// Each test context is independent, but descriptors are explicitly restored before disposal.
async function readAudit(page, names, heldNames = []) {
  return page.evaluateHandle(({ names, heldNames }) => {
    const descriptor = Object.getOwnPropertyDescriptor(File.prototype, "arrayBuffer");
    const original = File.prototype.arrayBuffer;
    const watched = new Set(names), held = new Set(heldNames);
    const state = {
      calls: [], completed: [], pending: [],
      restore() {
        if (descriptor) Object.defineProperty(File.prototype, "arrayBuffer", descriptor);
        else delete File.prototype.arrayBuffer;
      },
      async releaseAll() {
        const pending = state.pending.splice(0);
        await Promise.all(pending.map(release => release()));
        await Promise.resolve();
      },
    };
    Object.defineProperty(File.prototype, "arrayBuffer", {
      configurable: true, writable: true,
      value: function () {
        if (!watched.has(this.name)) return original.call(this);
        state.calls.push(this.name);
        const actual = async () => {
          const bytes = await original.call(this);
          state.completed.push(this.name);
          return bytes;
        };
        if (!held.has(this.name)) return actual();
        return new Promise((resolve, reject) => {
          state.pending.push(async () => {
            try { resolve(await actual()); }
            catch (error) { reject(error); }
          });
        });
      },
    });
    return state;
  }, { names, heldNames });
}
async function restoreAudit(handle) {
  try { await handle.evaluate(async state => { state.restore(); await state.releaseAll(); }); }
  finally { await handle.dispose(); }
}
async function calls(handle) { return handle.evaluate(state => state.calls.slice()); }
function assertNewItems(actual, expected, oldIds = []) {
  assert.equal(actual.length, expected.length);
  const ids = actual.map(item => item.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const id of ids) {
    assert.equal(typeof id, "string");
    assert.ok(id.length > 0 && !oldIds.includes(id), "each new payload gets a fresh local identity");
  }
  assert.deepEqual(actual.map(({ id, ...rest }) => rest), expected.map(value => ({
    revision: 1, kind: "text", ...value,
  })));
}

taskTextTest("confirmed multi-file text import atomically appends exact UTF-8 and preserves the existing model and undo", async f => {
  const original = await seed(f);
  const suffix = "An existing undoable edit.\n";
  await f.page.evaluate(value => {
    const editor = window.monaco.editor.getEditors()[0], model = editor.getModel();
    const end = model.getPositionAt(model.getValueLength());
    editor.pushUndoStop();
    editor.executeEdits("existing-task-edit", [{
      range: new window.monaco.Range(end.lineNumber, end.column, end.lineNumber, end.column), text: value,
    }]);
    editor.pushUndoStop();
  }, suffix);
  await f.current({ ...original.payload[0], text: original.payload[0].text + suffix });
  const previous = await f.export();
  const model = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
  const expected = [
    { filename: "説明😀.md", language: "markdown", text: "# 私人笔记\r\nExact Unicode: ☕ 🧩\n" },
    { filename: "module.rs", language: "rust", text: "fn main() {}\n" },
    { filename: "options.json", language: "json", text: '{"label":"授業"}\r\n' },
  ];
  const selected = expected.map((value, index) => file(value.filename, value.text,
    ["text/markdown", "application/octet-stream", "application/json"][index]));
  const audit = await readAudit(f.page, expected.map(value => value.filename));
  try {
    await chooseFiles(f, selected);
    await confirmation(f.page).waitFor();
    assert.deepEqual(await calls(audit), [], "selection and confirmation preview do not read content");
    for (const value of expected) await confirmation(f.page).getByText(value.filename, { exact: true }).waitFor();
    await approve(f.page).click();
    await confirmation(f.page).waitFor({ state: "detached" });
    assert.deepEqual(await calls(audit), expected.map(value => value.filename));
    const exported = await f.export();
    assert.equal(exported.title, previous.title);
    assert.equal(exported.format, previous.format); assert.equal(exported.version, previous.version);
    assert.deepEqual(exported.payload.slice(0, 1), previous.payload);
    assertNewItems(exported.payload.slice(1), expected, previous.payload.map(item => item.id));
    assert.equal(await f.item(previous.payload[0].id).getAttribute("aria-pressed"), "true");
    assert.equal(await model.evaluate(value => value === window.monaco.editor.getModels()[0] && !value.isDisposed()), true);
    await f.current(previous.payload[0]);
    await f.page.evaluate(() => window.monaco.editor.getEditors()[0].trigger("browser-test", "undo", null));
    await f.current(original.payload[0]);
    const undone = await f.export();
    assert.deepEqual(undone.payload[0], { ...previous.payload[0],
      revision: previous.payload[0].revision + 1, text: original.payload[0].text });
    assert.deepEqual(undone.payload.slice(1), exported.payload.slice(1));
  } finally { await restoreAudit(audit); await model.dispose(); }
});

taskTextTest("cancel reads nothing and two same-named files append as independent exact payloads after one fresh confirmation", async f => {
  await seed(f);
  const previous = await f.export();
  const selected = [file("same.txt", "First exact body.\n"),
    file("same.txt", "Second exact body: 中文\n", "application/octet-stream")];
  const audit = await readAudit(f.page, ["same.txt"]);
  try {
    await chooseFiles(f, selected);
    await confirmation(f.page).waitFor();
    assert.equal(await confirmation(f.page).getByText("same.txt", { exact: true }).count(), 2);
    assert.deepEqual(await calls(audit), []);
    await confirmation(f.page).getByRole("button", { name: "取消", exact: true }).click();
    await confirmation(f.page).waitFor({ state: "detached" });
    assert.deepEqual(await calls(audit), []);
    assert.deepEqual(await f.export(), previous);
    await confirmedFiles(f, selected);
    assert.deepEqual(await calls(audit), ["same.txt", "same.txt"]);
    const exported = await f.export();
    assert.equal(exported.title, previous.title);
    assert.deepEqual(exported.payload.slice(0, 1), previous.payload);
    assertNewItems(exported.payload.slice(1), [
      { filename: "same.txt", language: "plaintext", text: "First exact body.\n" },
      { filename: "same.txt", language: "plaintext", text: "Second exact body: 中文\n" },
    ], previous.payload.map(item => item.id));
    await f.current(previous.payload[0]);
  } finally { await restoreAudit(audit); }
});

taskTextTest("a later invalid UTF-8 file rejects the whole batch after a real first read without publishing its valid prefix", async f => {
  await seed(f);
  const previous = await f.export();
  const audit = await readAudit(f.page, ["valid-first.txt", "invalid-second.txt"], ["invalid-second.txt"]);
  try {
    await chooseFiles(f, [file("valid-first.txt", "A genuine valid prefix.\n"),
      file("invalid-second.txt", Buffer.from([0xc3, 0x28]))]);
    await confirmation(f.page).waitFor();
    assert.deepEqual(await calls(audit), []);
    await approve(f.page).click();
    await f.page.waitForFunction(state => state.completed.includes("valid-first.txt")
      && state.calls.includes("invalid-second.txt"), audit);
    assert.deepEqual(await f.ids(), previous.payload.map(item => item.id));
    assert.equal(await f.source(), previous.payload[0].text, "no intermediate new item enters the old editor");
    await audit.evaluate(async state => { await state.releaseAll(); });
    await confirmation(f.page).getByRole("alert").waitFor();
    await confirmation(f.page).getByRole("button", { name: "关闭", exact: true }).click();
    await confirmation(f.page).waitFor({ state: "detached" });
    assert.deepEqual(await calls(audit), ["valid-first.txt", "invalid-second.txt"]);
    assert.deepEqual(await f.export(), previous);
    await f.current(previous.payload[0]);
  } finally { await restoreAudit(audit); }
});

taskTextTest("31 plus two and a full 32-item task reject new files before confirmation or any native read", async f => {
  for (const count of [31, 32]) {
    const previous = taskDraft(Array.from({ length: count }, (_, index) => textItem(`kept_${index}`, {
      filename: `kept-${index}.txt`, text: `Existing ${index}.`, revision: 3,
    })), `Keep all ${count} items`);
    await f.import(previous);
    const audit = await readAudit(f.page, ["excess-a.txt", "excess-b.txt"]);
    try {
      assert.equal(await f.page.getByTestId("task-text-import-open").isDisabled(), count === 32);
      assert.equal(await f.page.getByTestId("task-text-import").isDisabled(), count === 32);
      // At 31 this is a normal picker selection. At 32 it defensively injects a change
      // into the disabled input; it is not evidence of an available user action.
      await chooseFiles(f, count === 31 ? [file("excess-a.txt", "One."), file("excess-b.txt", "Two.")]
        : [file("excess-a.txt", "No free slot.")]);
      // Flush the real change event/render, not a timer used to infer a background read.
      await f.page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
      assert.equal(await confirmation(f.page).count(), 0);
      assert.deepEqual(await calls(audit), []);
      assert.deepEqual(await f.export(), previous);
    } finally { await restoreAudit(audit); }
  }
});

taskTextTest("late genuine file reads cannot append after same-page anchors, owner navigation or confirmed logout", async f => {
  let logouts = 0;
  await f.page.context().route(`${engineUrl}/auth/logout`, route => {
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin,
      "access-control-allow-credentials": "true", "cache-control": "no-store" };
    if (route.request().method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    assert.equal(route.request().method(), "POST");
    assert.equal(route.request().postData(), null);
    logouts++;
    return route.fulfill({ headers, json: { ok: true } });
  });
  for (const transition of ["anchor", "navigation", "logout"]) {
    if (new URL(f.page.url()).pathname !== "/tasks/new") {
      await f.page.evaluate(() => window.next.router.push("/tasks/new"));
      await f.page.waitForURL(url => url.pathname === "/tasks/new");
      await f.page.getByTestId("task-title").waitFor();
    }
    await seed(f);
    const previous = await f.export();
    const filename = `late-${transition}.txt`;
    const audit = await readAudit(f.page, [filename], [filename]);
    const model = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
    try {
      await chooseFiles(f, [file(filename, "Late old-owner bytes must not enter a current task.\n")]);
      await confirmation(f.page).waitFor();
      assert.deepEqual(await calls(audit), []);
      await approve(f.page).click();
      await f.page.waitForFunction(state => state.calls.length === 1 && state.pending.length === 1, audit);
      if (transition === "anchor") {
        const authBefore = f.engineRequests.filter(request => request.path === "/auth/me").length;
        await f.page.evaluate(() => window.next.router.push("/tasks/new#text-import-closed"));
        await f.page.waitForURL(`${baseUrl}/tasks/new#text-import-closed`);
        await confirmation(f.page).waitFor({ state: "detached" });
        await audit.evaluate(async state => { await state.releaseAll(); });
        assert.deepEqual(await f.export(), previous);
        assert.equal(await model.evaluate(value => !value.isDisposed()
          && value === window.monaco.editor.getModels()[0]), true);
        assert.equal(f.engineRequests.filter(request => request.path === "/auth/me").length, authBefore);
        await f.current(previous.payload[0]);
      } else {
        if (transition === "navigation") {
          await f.page.evaluate(() => window.next.router.push("/account"));
          await f.page.waitForURL(`${baseUrl}/account`);
        } else {
          // A modal blocks ordinary pointer interaction. Probe the existing public logout handler
          // via its DOM button, not React state injection or a fabricated successful file result.
          await f.page.getByRole("button", { name: "退出登录", exact: true }).evaluate(button => button.click());
          await f.page.waitForURL(`${baseUrl}/login`);
          assert.equal(logouts, 1);
        }
        await f.page.waitForFunction(() => window.monaco?.editor.getModels().length === 0);
        assert.equal(await model.evaluate(value => value.isDisposed()), true);
        await f.page.evaluate(() => window.next.router.push("/tasks/new"));
        await f.page.waitForURL(url => url.pathname === "/tasks/new");
        await f.page.getByTestId("task-title").fill(`New owner after ${transition}`);
        await f.add(); await f.setSource("The newly owned payload stays exact.");
        const fresh = await f.export();
        await audit.evaluate(async state => { await state.releaseAll(); });
        assert.deepEqual(await f.export(), fresh);
        await f.current(fresh.payload[0]);
      }
      assert.deepEqual(await calls(audit), [filename]);
      assert.deepEqual(await audit.evaluate(state => state.completed), [filename],
        "release completed an actual native File read rather than a made-up result");
    } finally { await restoreAudit(audit); await model.dispose(); }
  }
});

taskTextTest("new text-import controls and confirmation fit 960 and 390 pixels and select the first item only for an empty draft", async f => {
  for (const width of [960, 390]) {
    await f.page.setViewportSize({ width, height: 900 });
    await f.page.getByTestId("task-title").fill(`Local authoring at ${width}`);
    const first = `${"n".repeat(105)}.future`;
    const text = "Exact local UTF-8: 中文 ☕\r\n";
    await chooseFiles(f, [file(first, text, "application/octet-stream"), file("notes.md", "# Second item\n")]);
    await confirmation(f.page).waitFor();
    await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
    await approve(f.page).click();
    await confirmation(f.page).waitFor({ state: "detached" });
    const exported = await f.export();
    assert.equal(exported.title, `Local authoring at ${width}`);
    assertNewItems(exported.payload, [
      { filename: first, language: "plaintext", text },
      { filename: "notes.md", language: "markdown", text: "# Second item\n" },
    ]);
    await f.current(exported.payload[0]);
    assert.equal(await f.item(exported.payload[0].id).getAttribute("aria-pressed"), "true");
    for (const locale of ["zh-CN", "en", "es", "ja"]) {
      await f.page.locator(".menu-toggle").click();
      await f.page.locator(".sidebar-footer select").selectOption(locale);
      await f.page.keyboard.press("Escape");
      await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth);
      assert.doesNotMatch(await f.page.locator("main").innerText(), /\b(?:taskDraft|editor|layout)\.[A-Za-z][\w.]*/);
      for (const id of ["task-text-import-open", "task-title", "task-draft-export", "editor-filename"]) {
        const box = await f.page.getByTestId(id).boundingBox();
        assert.ok(box && box.x >= 0 && box.x + box.width <= width, `${locale}: ${id} fits ${width}`);
      }
      assert.equal(await f.source(), text);
    }
    await f.page.locator(".menu-toggle").click();
    await f.page.locator(".sidebar-footer select").selectOption("zh-CN");
    await f.page.keyboard.press("Escape");
    await f.page.getByTestId("task-draft-reset").click();
    await approve(f.page).click();
    await confirmation(f.page).waitFor({ state: "detached" });
    assert.deepEqual(await f.export(), taskDraft());
  }
});
