import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { posix } from "node:path";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

// Only the repository's explicit test-only module forms are excluded. Unknown cfg forms,
// unreferenced files and production files whose names contain "test" remain guarded.
const testOnlyAttribute = /#\s*\[\s*cfg\s*\(\s*(?:test\s*|all\s*\((?:\w+\s*,\s*)*test\s*(?:,\s*\w+\s*)*\)\s*)\)\s*\]/;
const blank = text => text.replace(/[^\r\n]/g, " ");
// Keep byte positions for attributes/braces while ignoring comments and quoted braces/module text.
// This is a focused source guard, not a Rust parser or a replacement for compilation.
const syntaxOnly = source => source.replace(
  /\/\*[\s\S]*?\*\/|\/\/[^\n]*|\b(?:b|c)?r(#+)"[\s\S]*?"\1|"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\\n])'/g,
  blank,
);

function modules(path, source, available) {
  const syntax = syntaxOnly(source);
  const ranges = [], references = [];
  const declarations = /((?:#\s*\[[^\]]*\]\s*)*)(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{])/g;
  for (const match of syntax.matchAll(declarations)) {
    const attrs = source.slice(match.index, match.index + match[1].length);
    const testOnly = testOnlyAttribute.test(attrs)
      || ranges.some(([start, end]) => match.index > start && match.index < end);
    if (match[3] === "{") {
      if (!testOnly) continue;
      const opening = match.index + match[0].length - 1;
      let end = opening + 1, depth = 1;
      for (; end < syntax.length && depth; end++) {
        if (syntax[end] === "{") depth++;
        else if (syntax[end] === "}") depth--;
      }
      assert.equal(depth, 0, `test module braces must balance in ${path}`);
      ranges.push([match.index, end]);
      continue;
    }
    const directory = posix.dirname(path);
    const explicit = attrs.match(/#\s*\[\s*path\s*=\s*"([^"\\]+)"\s*\]/);
    // Unsupported path attributes are not guessed; their files remain in the complete scan.
    if (!explicit && /#\s*\[\s*path\b/.test(attrs)) continue;
    const base = posix.basename(path) === "mod.rs" ? directory : path.replace(/\.rs$/, "");
    const candidates = explicit ? [posix.join(directory, explicit[1])]
      : [`${base}/${match[2]}.rs`, `${base}/${match[2]}/mod.rs`];
    const target = candidates.find(candidate => available.has(candidate));
    if (target) references.push({ target, testOnly });
  }
  let production = source;
  for (const [start, end] of ranges.sort(([a], [b]) => b - a)) {
    production = production.slice(0, start) + blank(production.slice(start, end)) + production.slice(end);
  }
  return { production, references };
}

function productionRustSources(files, entry = "engine/src/platform_http/mod.rs") {
  const available = new Map(files);
  assert.ok(available.has(entry), "the product module root must be present");
  const parsed = new Map(files.map(([path, source]) => [path, modules(path, source, available)]));
  const excluded = new Set();
  const pending = [...parsed.values()].flatMap(value => value.references.filter(ref => ref.testOnly).map(ref => ref.target));
  while (pending.length) {
    const path = pending.pop();
    if (excluded.has(path)) continue;
    excluded.add(path);
    pending.push(...parsed.get(path).references.map(ref => ref.target));
  }
  // A shared test/product file is still product. Seed the real root and conservatively retain
  // every file not explicitly reached through a test module; then propagate production edges.
  excluded.delete(entry);
  let changed = true;
  while (changed) {
    changed = false;
    for (const [path, value] of parsed) {
      if (excluded.has(path)) continue;
      for (const ref of value.references) {
        if (!ref.testOnly && excluded.delete(ref.target)) changed = true;
      }
    }
  }
  return files.filter(([path]) => !excluded.has(path)).map(([path]) => [path, parsed.get(path).production]);
}

test("prepared HTTP production scan excludes only explicit cfg(test) module trees", () => {
  const files = [
    ["http/mod.rs", `mod live;
      #[cfg(test)] mod fixtures;
      #[cfg(test)] #[path = "checks.rs"] mod checks;
      #[cfg(all(test, unix))] mod unix_only;`],
    ["http/live.rs", `#[path = "shared.rs"] mod shared;
      fn before() { try_acquire_owned(); }
      #[cfg(test)] mod tests {
        fn controlled_work() { tokio::spawn(async { let braces = r#"{ } mod phantom;"#; }); }
      }
      fn after() { retained_product(); }`],
    ["http/fixtures.rs", `mod workers; #[path = "shared.rs"] mod shared; tokio::spawn(async {});`],
    ["http/fixtures/workers.rs", "tokio::spawn_blocking(|| {});"],
    ["http/checks.rs", `#[path = "checks/nested.rs"] mod nested;`],
    ["http/checks/nested.rs", "semaphore.acquire().await;"],
    ["http/unix_only.rs", "std::env::var(\"TEST\");"],
    ["http/shared.rs", "fn shared_product() {}"],
    ["http/orphan_tests.rs", "fn still_product() {}"],
  ];
  const production = productionRustSources(files, "http/mod.rs");
  assert.deepEqual(production.map(([path]) => path), ["http/mod.rs", "http/live.rs", "http/shared.rs", "http/orphan_tests.rs"]);
  const live = production.find(([path]) => path === "http/live.rs")[1];
  assert.match(live, /fn before\(\)[\s\S]*fn after\(/);
  assert.doesNotMatch(live, /tokio::spawn|phantom/);
});

test("prepared HTTP production scan never exempts production work by name or uncertain cfg", () => {
  for (const forbidden of ["semaphore.acquire().await;", "tokio::spawn(async {});", "spawn_blocking(|| {});", "std::env::var(\"X\");"]) {
    const files = [
      ["http/mod.rs", `#[cfg(any(test, unix))] mod tests;`],
      ["http/tests.rs", `fn production() { ${forbidden} }`],
      ["http/unreferenced_test_support.rs", forbidden],
    ];
    const production = productionRustSources(files, "http/mod.rs");
    assert.equal(production.length, 3);
    assert.throws(() => assert.doesNotMatch(production.map(([, source]) => codeOnly(source)).join("\n"),
      /\.acquire(?:_owned)?\s*\(|\b(?:spawn|spawn_blocking)\s*\(|\b(?:std|tokio)::env\b/));
  }
});

test("prepared HTTP production scan retains every Session login product module", async () => {
  const files = await sources("engine/src/platform_http", /\.rs$/);
  const product = new Map(productionRustSources(files));
  for (const name of ["mod", "handlers", "headers", "rate"]) {
    assert.ok(product.has(`engine/src/platform_http/session_login/${name}.rs`));
  }
  for (const name of ["test_support.rs", "tests.rs", "tests/bodies.rs", "tests/results.rs", "tests/rate.rs"]) {
    assert.ok(!product.has(`engine/src/platform_http/session_login/${name}`));
  }
  assert.doesNotMatch(product.get("engine/src/platform_http/session_login/rate.rs"), /fn global_login_rate_/);
});

// These are wiring guards, not evidence that a semaphore, body or transaction behaves correctly.
// The focused Rust/HTTP tests exercise sharing, saturation, cancellation and response lifetime.
test("prepared HTTP capacity does not mount the router or expose browser and SDK transport", async () => {
  const files = await Promise.all([
    "engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json",
  ].map(async path => [path, await read(path)]));
  for (const [directory, extension] of [
    ["engine/src/routes", /\.rs$/], ["sdk-js/src", /\.ts$/],
    ["frontend/src/features/tasks", /\.tsx?$/],
  ]) files.push(...await sources(directory, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source),
    /\b(?:TaskContentHttpState|build_task_content_router)\b|\/platform\/v1\/tasks/, path);
});

test("prepared HTTP capacity is bounded trusted configuration shared by state clones", async () => {
  const state = codeOnly(await read("engine/src/platform_http/mod.rs"));
  assert.match(state, /#\[derive\(Clone\)\]\s*pub struct TaskContentHttpState/);
  assert.match(state, /Arc\s*<\s*Semaphore\s*>/);
  assert.match(state, /pub fn with_capacity\([\s\S]*?max_in_flight:\s*usize/);
  assert.match(state, /config::validate\(origin, &\[max_in_flight\]\)\?;/);
  const configuration = codeOnly(await read("engine/src/platform_http/config.rs"));
  assert.match(configuration, /security::validate_origin\(origin\)\?;[\s\S]*capacities\s*\.iter\(\)\s*\.any\([\s\S]*!\(1\s*\.\.=\s*16\)\.contains\(/);
  assert.ok(state.indexOf("config::validate(origin, &[max_in_flight])?") < state.indexOf("Semaphore::new(max_in_flight)"));
  assert.match(state, /Self::with_capacity\([\s\S]*?,\s*4\s*\)/,
    "the compatibility constructor keeps a bounded default of four");
  assert.match(state, /Semaphore::new\(max_in_flight\)/);
  const all = productionRustSources(await sources("engine/src/platform_http", /\.rs$/))
    .map(([, source]) => codeOnly(source)).join("\n");
  assert.match(all, /try_acquire_owned\(/);
  assert.doesNotMatch(all, /\.acquire(?:_owned)?\s*\(|\b(?:spawn|spawn_blocking)\s*\(|\b(?:std|tokio)::env\b/,
    "admission must not add a waiting queue, detached work or ambient configuration");
});

test("prepared HTTP admission precedes body work and keeps its permit in the response body", async () => {
  const handler = codeOnly(await read("engine/src/platform_http/handlers.rs"));
  const admission = codeOnly(await read("engine/src/platform_http/admission.rs"));
  const all = productionRustSources(await sources("engine/src/platform_http", /\.rs$/))
    .map(([, source]) => codeOnly(source)).join("\n");
  assert.match(handler, /endpoint\(&request\)[\s\S]*credentials\([\s\S]*admission::acquire\([\s\S]*handle\([\s\S]*admission::response\(/);
  assert.match(admission, /fn acquire\([\s\S]*try_acquire_owned\(/);
  assert.match(handler.slice(handler.indexOf("async fn handle(")), /request_bytes\([\s\S]*match endpoint/);
  assert.match(all, /HttpError::new\(\s*StatusCode::SERVICE_UNAVAILABLE,\s*"capacity_exhausted"\s*,?\s*\)/,
    "capacity rejection uses the pre-dispatch not_attempted constructor");
  assert.match(all, /Option\s*<\s*OwnedSemaphorePermit\s*>/,
    "the body owns the permit and can release it at a terminal frame");
  assert.match(all, /impl\s+(?:[\w:]+::)?(?:HttpBody|Body)\s+for\s+\w+/);
  const polling = admission.slice(admission.indexOf("fn poll_frame("), admission.indexOf("fn is_end_stream("));
  for (const pattern of [/Poll::Ready\(/, /\bNone\b/, /\bErr\s*\(/, /\.take\(\)/,
    /\.is_end_stream\(\)/]) assert.match(polling, pattern);
  assert.doesNotMatch(handler, /\b(?:timeout|timeout_at|sleep|spawn|spawn_blocking)\s*\(|RETRY_AFTER|Retry-After/,
    "capacity does not add a competing mutation deadline or automatic retry");
});
