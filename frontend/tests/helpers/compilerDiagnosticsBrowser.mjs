import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import nextWebpack from "next/dist/compiled/webpack/webpack.js";

export async function buildFixture(entry = new URL("../fixtures/compilerDiagnostics.mjs", import.meta.url)) {
  const directory = await mkdtemp(join(tmpdir(), "cyanrex-diagnostics-browser-"));
  nextWebpack.init();
  const compiler = nextWebpack.webpack({
    mode: "development", devtool: false,
    entry: fileURLToPath(entry),
    output: { path: directory, filename: "fixture.js" },
    resolve: { extensions: [".ts", ".js", ".mjs"] },
    plugins: [new nextWebpack.webpack.DefinePlugin({ "process.env.NEXT_PUBLIC_ENGINE_URL": JSON.stringify("https://engine-a.invalid") })],
    module: { rules: [{ test: /\.ts$/, use: fileURLToPath(new URL("typescript-loader.cjs", import.meta.url)) }] },
  });
  try {
    await new Promise((resolve, reject) => compiler.run((error, stats) => {
      if (error || stats.hasErrors()) reject(error || new Error(stats.toString({ all: false, errors: true })));
      else resolve();
    }));
    return await readFile(join(directory, "fixture.js"), "utf8");
  } finally {
    await new Promise((resolve, reject) => compiler.close(error => error ? reject(error) : resolve()));
    await rm(directory, { recursive: true, force: true });
  }
}

export async function setupFixture(browser, bundle) {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.route("**/*", route => { errors.push("Unexpected network request"); return route.abort(); });
  await page.clock.install({ time: new Date("2026-09-13T00:00:00Z") });
  await page.clock.pauseAt(new Date("2026-09-13T00:00:01Z"));
  await page.setContent('<main id="root"></main>');
  await page.addScriptTag({ content: bundle });
  return {
    page, errors,
    render: (editors = [{}]) => page.evaluate(items => window.fixture.render(items), editors),
    advance: (ms = 700) => page.clock.runFor(ms),
    respond: (index, payload, status = 200) => page.evaluate(args => window.fixture.respond(...args), [index, payload, status]),
    reject: index => page.evaluate(i => window.fixture.requests[i].reject(new TypeError("synthetic failure")), index),
    snapshot: (id = "editor") => page.locator(`[data-editor="${id}"]`).evaluate(el => JSON.parse(el.textContent)),
    requests: () => page.evaluate(() => window.fixture.requests.map(({ url, init }) => ({
      url, method: init.method || "GET", body: init.body && JSON.parse(init.body), aborted: init.signal?.aborted,
      cache: init.cache, redirect: init.redirect, credentials: init.credentials,
    }))),
    close: () => page.close(),
  };
}
