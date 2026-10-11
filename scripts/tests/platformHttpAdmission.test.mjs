import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
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
  assert.match(state, /(?:1\s*\.\.=\s*16|MAX_[A-Z_]+\s*:\s*usize\s*=\s*16)/);
  assert.match(state, /Self::with_capacity\([\s\S]*?,\s*4\s*\)/,
    "the compatibility constructor keeps a bounded default of four");
  assert.match(state, /Semaphore::new\(max_in_flight\)/);
  const all = (await sources("engine/src/platform_http", /\.rs$/))
    .map(([, source]) => codeOnly(source)).join("\n");
  assert.match(all, /try_acquire_owned\(/);
  assert.doesNotMatch(all, /\.acquire(?:_owned)?\s*\(|\b(?:spawn|spawn_blocking)\s*\(|\b(?:std|tokio)::env\b/,
    "admission must not add a waiting queue, detached work or ambient configuration");
});

test("prepared HTTP admission precedes body work and keeps its permit in the response body", async () => {
  const handler = codeOnly(await read("engine/src/platform_http/handlers.rs"));
  const admission = codeOnly(await read("engine/src/platform_http/admission.rs"));
  const all = (await sources("engine/src/platform_http", /\.rs$/))
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
