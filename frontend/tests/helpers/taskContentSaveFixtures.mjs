import assert from "node:assert/strict";
import { createHash } from "node:crypto";

export const uuid = value => `00000000-0000-4000-8000-${value.toString(16).padStart(12, "0")}`;
export const clone = value => structuredClone(value);
export const created = "2026-10-26T01:02:03Z";
export const updated = "2026-10-26T02:03:04Z";
export const workspace = () => ({ authority_id: uuid(1), workspace_id: uuid(2) });
export const owner = () => ({ authority_id: uuid(1), principal_id: uuid(4) });
export const item = (patch = {}) => ({ id: "local_only", revision: 7, kind: "text",
  filename: "notes.txt", language: "plaintext", text: "讨论记录，不需要代码或 Run。😀\n", ...patch });
export const draft = (payload = [item()], patch = {}) => ({ format: "cyanrex.task-draft",
  version: 1, title: "Updated collaboration task", payload, ...patch });
export const guard = (patch = {}) => ({ generation: 2, revision: 8, contextEpoch: 3, ...patch });
export const allocations = count => Array.from({ length: count }, (_, index) => ({
  artifact_id: uuid(100 + index), revision_id: uuid(200 + index),
}));
export function baseline(payload = []) {
  const manifest = { schema_version: 1, title: "Original collaboration task", payload };
  return { task: { schema_version: 1, reference: { workspace: workspace(), task_id: uuid(3) },
    owner: owner(), title: manifest.title, definition: null,
    input_refs: payload.map(value => clone(value.artifact)), status: "draft", revision: 5,
    created_at: created, updated_at: created }, manifest };
}
export const digest = text => createHash("sha256").update(text, "utf8").digest("hex");
export function reference(allocation, text) {
  return { workspace: workspace(), ...allocation, sha256: digest(text) };
}
export function artifactResponse(step) {
  return { schema_version: 1, artifact: { schema_version: 1,
    reference: reference({ artifact_id: step.body.artifact_id, revision_id: step.body.revision_id }, step.body.text),
    owner: owner(), sequence: 1, parent: null, kind: "cyanrex.task-text", title: step.body.title,
    media_type: "text/plain", byte_length: Buffer.byteLength(step.body.text, "utf8"), created_at: updated } };
}
export function taskResponse(step, original = baseline()) {
  const manifest = clone(step.body.manifest);
  return { schema_version: 1, task: { ...clone(original.task), title: manifest.title,
    input_refs: manifest.payload.map(value => clone(value.artifact)),
    revision: original.task.revision + 1, updated_at: updated }, manifest };
}
export function assertFrozen(value) {
  if (!value || typeof value !== "object") return;
  assert.ok(Object.isFrozen(value), "all outward object/array values must be frozen");
  for (const child of Object.values(value)) assertFrozen(child);
}
export function assertProgress(attempt, expected) {
  const result = attempt.progress();
  assert.deepEqual(result, { state: "ready", confirmedArtifacts: 0,
    taskConfirmed: false, pending: null, reason: null, ...expected });
  assertFrozen(result);
  return result;
}
export async function prepare(prepareTaskContentSave, options = {}) {
  const source = Object.hasOwn(options, "draft") ? options.draft : draft();
  const original = Object.hasOwn(options, "baseline") ? options.baseline : baseline();
  const current = Object.hasOwn(options, "guard") ? options.guard : guard();
  const targets = Object.hasOwn(options, "allocations") ? options.allocations : allocations(source.payload.length);
  const attempt = await prepareTaskContentSave(source, original, current, targets);
  return { attempt, draft: source, baseline: original, guard: current, allocations: targets };
}
