import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { registerHooks } from "node:module";
import { setupTask, taskDraft, textItem } from "./helpers/taskPayloadEditorBrowser.mjs";

const localeRoot = new URL("../src/i18n/", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, next) {
  return context.parentURL?.startsWith(localeRoot) && specifier.startsWith(".")
    && !/\.[cm]?[jt]sx?$/.test(specifier)
    ? next(new URL(`${specifier}.ts`, context.parentURL).href, context) : next(specifier, context);
} });
let translations;
try { ({ translations } = await import("../src/i18n/translations.ts")); }
finally { resolver.deregister(); }
const translated = (locale, key) => {
  const value = key.split(".").reduce((dict, part) => dict?.[part], translations[locale]);
  assert.equal(typeof value, "string", `${locale}.${key} is explicit, not fallback`);
  return value;
};

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

function layoutTest(name, body, options) {
  test(name, { timeout: 90000 }, async () => {
    const f = await setupTask(browser, options);
    let failure;
    const record = error => { failure = failure ? new AggregateError([failure, error], "Layout behavior and cleanup failed") : error; };
    try { await body(f); } catch (error) { record(error); }
    finally {
      try { await f.assertLocalOnly(); } catch (error) { record(error); }
      finally { try { await f.close(); } catch (error) { record(error); } }
    }
    if (failure) throw failure;
  });
}
const sourceSurface = page => page.locator(".language-editor-surface");
async function box(locator, label) {
  const value = await locator.boundingBox();
  assert.ok(value && value.width > 0 && value.height > 0, `${label} has actual nonzero geometry`);
  return value;
}
async function layoutReady(page) {
  await page.waitForFunction(() => {
    const surface = document.querySelector(".language-editor-surface");
    const editor = window.monaco?.editor.getEditors()[0];
    return surface && editor && editor.getLayoutInfo().width > 0
      && Math.abs(editor.getLayoutInfo().width - surface.clientWidth) <= 2;
  });
}
async function fitAndHit(page, locator, label) {
  await locator.scrollIntoViewIfNeeded();
  const actual = await locator.evaluate(element => {
    const r = element.getBoundingClientRect();
    return { x: r.x, right: r.right, height: r.height, width: r.width, viewport: innerWidth,
      hit: element.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)) };
  });
  assert.ok(actual.width > 0 && actual.height >= 40, `${label} is at least 40px high`);
  assert.ok(actual.x >= 0 && actual.right <= actual.viewport + 1, `${label} fits horizontally`);
  assert.equal(actual.hit, true, `${label} center is not clipped or covered`);
}
async function seed(f, overrides = {}, title = "Layout task") {
  const item = textItem("layout_owned_text", { revision: 7, filename: "kept.ts", language: "typescript",
    text: "const count: number = 7;\n", ...overrides });
  const draft = taskDraft([item], title);
  await f.import(draft); await f.current(item); await layoutReady(f.page);
  return draft;
}
async function remember(page) {
  const model = await page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
  let editor;
  try {
    editor = await page.evaluateHandle(() => window.monaco.editor.getEditors()[0]);
    const initial = await model.evaluate(value => ({ uri: value.uri.toString(), version: value.getVersionId(),
      text: value.getValue(), language: value.getLanguageId(), undo: value.canUndo() }));
    const sameOwner = async () => {
      assert.equal(await model.evaluate(value => !value.isDisposed() && value === window.monaco.editor.getModels()[0]), true);
      assert.equal(await editor.evaluate(value => value === window.monaco.editor.getEditors()[0]), true);
      assert.equal(await model.evaluate(value => value.uri.toString()), initial.uri);
      assert.equal(await page.evaluate(() => window.monaco.editor.getModels().length), 1);
    };
    return {
      sameOwner,
      async unchanged() {
        await sameOwner();
        assert.deepEqual(await model.evaluate(value => ({ uri: value.uri.toString(), version: value.getVersionId(),
          text: value.getValue(), language: value.getLanguageId(), undo: value.canUndo() })), initial);
      },
      async close() { try { await editor.dispose(); } finally { await model.dispose(); } },
    };
  } catch (error) { await editor?.dispose(); await model.dispose(); throw error; }
}

layoutTest("desktop task editing starts compactly and responsive resizing preserves the real model editor and undo", async f => {
  assert.equal(await f.page.locator(".monaco-editor").count(), 0);
  const original = await seed(f);
  await f.page.evaluate(() => window.scrollTo(0, 0));
  const summary = await box(f.page.locator(".task-draft-summary"), "task summary");
  const surface = await box(sourceSurface(f.page), "editor surface");
  assert.ok(summary.height <= 130, `desktop summary is ${summary.height}px, not compact`);
  assert.ok(surface.y <= 440, `desktop source begins at ${surface.y}px`);
  const capabilities = await box(f.page.getByTestId("editor-capabilities"), "capability explanation");
  assert.ok(capabilities.y >= surface.y + surface.height - 1, "capability explanation follows the source surface");
  await f.page.evaluate(() => {
    const editor = window.monaco.editor.getEditors()[0];
    editor.pushUndoStop();
    editor.executeEdits("task-layout-test", [{ range: new window.monaco.Range(1, 1, 1, 1), text: "// undoable edit\n" }]);
    editor.pushUndoStop();
  });
  await f.current({ ...original.payload[0], text: "// undoable edit\n" + original.payload[0].text });
  const previous = await f.export(), identity = await remember(f.page);
  try {
    assert.equal(await f.page.evaluate(() => window.monaco.editor.getModels()[0].canUndo()), true);
    for (const width of [1440, 960, 390, 320, 1440]) {
      await f.page.setViewportSize({ width, height: 900 });
      await layoutReady(f.page); await identity.unchanged();
      assert.deepEqual(await f.export(), previous);
    }
    await f.page.evaluate(() => window.monaco.editor.getEditors()[0].trigger("task-layout-test", "undo", null));
    await f.current(original.payload[0]);
    assert.deepEqual(await f.export(), { ...previous, payload: [{ ...previous.payload[0],
      revision: previous.payload[0].revision + 1, text: original.payload[0].text }] });
    await identity.sameOwner();
  } finally { await identity.close(); }
});

layoutTest("compact mobile task controls remain reachable and the normal Chinese source starts within the first 900px", async f => {
  const original = await seed(f, {}, "短任务");
  for (const width of [390, 320]) {
    await f.page.setViewportSize({ width, height: 900 });
    await layoutReady(f.page);
    await f.page.evaluate(() => window.scrollTo(0, 0));
    const surface = await box(sourceSurface(f.page), `${width}px source`);
    assert.ok(surface.y <= 900, `${width}px source starts at ${surface.y}px`);
    assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    for (const id of ["task-title", "task-payload-add", "task-text-import-open", "task-draft-export",
      "task-draft-reset", "editor-filename", "language-select", "editor-download", "editor-reset"]) {
      await fitAndHit(f.page, f.page.getByTestId(id), `${width}px ${id}`);
    }
    for (const button of await f.page.locator(".task-draft-actions button").all()) await fitAndHit(f.page, button, `${width}px task action`);
    assert.equal(await f.page.getByText(translated("zh-CN", "taskDraft.localOnly"), { exact: true }).isVisible(), true);
    assert.equal(await f.page.getByTestId("editor-capabilities").isVisible(), true);
    await f.current(original.payload[0]);
  }
  assert.deepEqual(await f.export(), original);
}, { viewport: { width: 390, height: 900 } });

layoutTest("a full 32-item long-filename collection scrolls independently on mobile without losing items or selection", async f => {
  const original = taskDraft(Array.from({ length: 32 }, (_, index) => textItem(`collection_${index}`, {
    revision: 3, filename: `${String(index).padStart(2, "0")}-${"n".repeat(122)}.ts`,
    language: "typescript", text: `const count: number = ${index};\n`,
  })), "Keep all 32 local text items");
  assert.ok(original.payload.every(item => item.filename.length === 128));
  await f.import(original); await f.current(original.payload[0]); await layoutReady(f.page);
  const list = f.page.getByTestId("task-payload-list");
  const geometry = await box(list, "mobile collection");
  assert.ok(geometry.height <= 210, `mobile collection takes ${geometry.height}px`);
  assert.equal(await list.evaluate(element => element.scrollHeight > element.clientHeight), true);
  assert.deepEqual(await f.ids(), original.payload.map(item => item.id));
  const last = original.payload.at(-1), target = f.item(last.id);
  await target.scrollIntoViewIfNeeded();
  assert.equal(await list.evaluate(element => element.scrollTop > 0), true, "the collection itself scrolls to the last item");
  await fitAndHit(f.page, target, "last long-filename item");
  assert.equal(await target.locator(".task-payload-filename").textContent(), last.filename);
  await target.click(); await f.current(last);
  assert.equal(await target.getAttribute("aria-pressed"), "true");
  assert.equal(await list.locator('button[aria-pressed="true"]').count(), 1);
  for (const id of ["task-payload-add", "task-text-import-open", "task-text-import"]) {
    assert.equal(await f.page.getByTestId(id).isDisabled(), true, id);
  }
  assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  assert.deepEqual(await f.export(), original, "scrolling and selecting consume no payload revision");
}, { viewport: { width: 390, height: 900 } });

layoutTest("all five locales fit long task metadata and the short mobile menu keeps its close button visible without replacing the model", async f => {
  const original = await seed(f, { filename: `${"p".repeat(125)}.ts` }, `Long task title ${"t".repeat(180)}`);
  const identity = await remember(f.page);
  try {
    for (const locale of ["zh-CN", "en", "es", "ja", "ko"]) {
      await f.page.setViewportSize({ width: 390, height: 900 });
      const menu = f.page.locator(".menu-toggle");
      await menu.click();
      await f.page.locator(".sidebar-footer select").selectOption(locale);
      await f.page.waitForFunction(value => document.documentElement.lang === value, locale);
      await f.page.locator(".sidebar-footer select").focus();
      await f.page.keyboard.press("Escape");
      assert.equal(await menu.getAttribute("aria-expanded"), "false");
      assert.equal(await menu.getAttribute("aria-label"), translated(locale, "layout.menu"));
      assert.equal(await menu.getAttribute("aria-controls"), "workspace-navigation");
      assert.equal(await menu.evaluate(element => element === document.activeElement), true);
      for (const width of [390, 320]) {
        await f.page.setViewportSize({ width, height: 900 }); await layoutReady(f.page);
        assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), locale);
        for (const id of ["task-title", "task-text-import-open", "task-draft-export", "editor-filename", "language-select", "editor-download"]) {
          await fitAndHit(f.page, f.page.getByTestId(id), `${locale} ${width}px ${id}`);
        }
        assert.equal(await f.page.getByTestId("task-draft-export").textContent(), translated(locale, "taskDraft.export"));
        assert.equal(await f.page.getByText(translated(locale, "taskDraft.localOnly"), { exact: true }).isVisible(), true);
        assert.doesNotMatch(await f.page.locator("main").innerText(), /\b(?:taskDraft|editorIntelligence|editor|layout)\.[A-Za-z][\w.]*/);
        await identity.unchanged();
      }
      await f.page.setViewportSize({ width: 390, height: 420 });
      await f.page.evaluate(() => window.scrollTo(0, 0));
      await menu.click();
      assert.equal(await menu.getAttribute("aria-expanded"), "true");
      const language = f.page.locator(".sidebar-footer select");
      await language.scrollIntoViewIfNeeded();
      await language.focus();
      const navigation = f.page.locator("#workspace-navigation");
      assert.equal(await navigation.evaluate(element => element.scrollHeight > element.clientHeight && element.scrollTop > 0), true,
        "the opened navigation body scrolls rather than moving its close control off-screen");
      const close = await box(menu, `${locale} short-viewport menu toggle`);
      assert.ok(close.y >= 0 && close.y + close.height <= 80, `${locale}: menu close control remains within the first 80px`);
      await fitAndHit(f.page, menu, `${locale} short-viewport menu toggle`);
      await f.page.keyboard.press("Escape");
      assert.equal(await menu.getAttribute("aria-expanded"), "false");
      assert.equal(await menu.evaluate(element => element === document.activeElement), true);
      await layoutReady(f.page); await identity.unchanged();
      assert.deepEqual(await f.export(), original, `${locale}: layout changes are not edits`);
    }
  } finally { await identity.close(); }
}, { viewport: { width: 390, height: 900 } });
