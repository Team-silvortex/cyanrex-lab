import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { setupTask, confirmation, approve } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

test("text imports safely bound supplementary filename characters without rejecting valid content", { timeout: 60000 }, async () => {
  const f = await setupTask(browser);
  try {
    await f.add();
    const cases = [
      { filename: `${"a".repeat(127)}😀.txt`, bounded: "a".repeat(127), text: "Exact imported text: 中文 ☕\r\n" },
      { filename: `${"a".repeat(126)}😀.txt`, bounded: `${"a".repeat(126)}😀`, text: "The complete final character remains.\n" },
    ];
    for (const { filename, bounded, text } of cases) {
      const before = await f.export();
      await f.page.getByTestId("editor-import").setInputFiles({
        name: filename, mimeType: "text/plain", buffer: Buffer.from(text),
      });
      await confirmation(f.page).waitFor();
      await approve(f.page).click();
      await f.page.waitForFunction(() => !document.querySelector(".safety-dialog")
        || Boolean(document.querySelector(".safety-dialog [role=alert]")));
      const failure = confirmation(f.page).getByRole("alert");
      assert.equal(await failure.count(), 0, "a valid filename must not cause the content import to fail");
      await confirmation(f.page).waitFor({ state: "detached" });
      await f.current({ filename: bounded, language: "plaintext", text });
      const exported = await f.export();
      assert.deepEqual(exported.payload, [{
        ...before.payload[0], revision: before.payload[0].revision + 1,
        filename: bounded, language: "plaintext", text,
      }]);
      const pending = f.page.waitForEvent("download");
      await f.page.getByTestId("editor-download").click();
      const download = await pending, chunks = [];
      for await (const chunk of await download.createReadStream()) chunks.push(chunk);
      // Chromium may add the Blob's .txt extension after truncation removed the original extension.
      assert.ok([bounded, `${bounded}.txt`].includes(download.suggestedFilename()));
      assert.deepEqual(Buffer.concat(chunks), Buffer.from(text));
    }
    await f.assertLocalOnly();
  } finally { await f.close(); }
});
