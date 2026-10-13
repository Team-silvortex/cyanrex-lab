import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { setupTask, taskDraft, textItem } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

test("locale changes update actual snippets accessibility and plain-text labels without changing the task model or undo", { timeout: 90000 }, async () => {
  const f = await setupTask(browser);
  let failure, model;
  try {
    const original = taskDraft([textItem("localized_local_item", {
      revision: 7, filename: "kept.rs", language: "rust", text: "// Private local source\nfn",
    })], "Preserve this local task");
    await f.import(original);
    await f.current(original.payload[0]);
    model = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
    const inserted = "// One undoable edit\n";
    await f.page.evaluate(text => {
      const editor = window.monaco.editor.getEditors()[0];
      editor.pushUndoStop();
      editor.executeEdits("localization-browser-edit", [{
        range: new window.monaco.Range(1, 1, 1, 1), text,
      }]);
      editor.pushUndoStop();
    }, inserted);
    await f.current({ ...original.payload[0], text: inserted + original.payload[0].text });
    const before = await f.export();
    const editor = await f.page.evaluateHandle(() => window.monaco.editor.getEditors()[0]);
    try {
      const expected = [
        { locale: "zh-CN", plain: "纯文本", aria: "代码编辑器", snippet: "Rust 函数片段" },
        { locale: "en", plain: "Plain text", aria: "Code editor", snippet: "Rust function snippet" },
        { locale: "es", plain: "Texto sin formato", aria: "Editor de código", snippet: "Fragmento de función de Rust" },
        { locale: "ja", plain: "プレーンテキスト", aria: "コードエディター", snippet: "Rust 関数スニペット" },
      ];
      for (const value of expected) {
        await f.page.locator(".sidebar-footer select").selectOption(value.locale);
        await f.page.waitForFunction(label => window.monaco.editor.getEditors()[0].getRawOptions().ariaLabel === label, value.aria);
        assert.equal(await f.page.getByTestId("language-select").locator('option[value="plaintext"]').textContent(), value.plain);
        assert.match(await f.page.locator('.monaco-editor [role="textbox"]').getAttribute("aria-label"),
          new RegExp(value.aria.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
        assert.equal(await model.evaluate(value => !value.isDisposed() && value === window.monaco.editor.getModels()[0]), true);
        assert.equal(await editor.evaluate(value => value === window.monaco.editor.getEditors()[0]), true);
        assert.deepEqual(await f.export(), before, "changing display locale never edits task title, identity, revision or text");
        await f.page.evaluate(() => {
          const current = window.monaco.editor.getEditors()[0];
          current.setPosition(current.getModel().getPositionAt(current.getModel().getValueLength()));
        });
        await f.page.getByTestId("editor-completion").click();
        await f.page.locator(".suggest-widget .monaco-list-row").filter({ hasText: value.snippet }).waitFor();
        assert.equal(await f.source(), before.payload[0].text, "showing localized suggestions does not insert code");
        await f.page.keyboard.press("Escape");
      }
      await f.page.evaluate(() => window.monaco.editor.getEditors()[0].trigger("localization-browser-test", "undo", null));
      await f.current(original.payload[0]);
      assert.deepEqual(await f.export(), { ...before, payload: [{ ...before.payload[0],
        revision: before.payload[0].revision + 1, text: original.payload[0].text }] });
      assert.equal(await model.evaluate(value => !value.isDisposed() && value === window.monaco.editor.getModels()[0]), true);
    } finally { await editor.dispose(); }
  } catch (error) { failure = error; }
  finally {
    try {
      try { await f.assertLocalOnly(); }
      catch (error) { failure = failure ? new AggregateError([failure, error], "Behavior and isolation failed") : error; }
      finally { await model?.dispose(); }
    } finally { await f.close(); }
  }
  if (failure) throw failure;
});
