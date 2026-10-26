/** Pure supplied-value agreement, not authenticated responses or reusable authority. */
import { isTaskPayloadLanguage } from "./taskPayloadLanguage";

export type Workspace = Readonly<{ authority_id: string; workspace_id: string }>;
export type Principal = Readonly<{ authority_id: string; principal_id: string }>;
export type ArtifactReference = Readonly<{
  workspace: Workspace; artifact_id: string; revision_id: string; sha256: string;
}>;
export type Binding = Readonly<{
  kind: "text"; filename: string; language: string; artifact: ArtifactReference;
}>;
export type Manifest = Readonly<{ schema_version: 1; title: string; payload: readonly Binding[] }>;
export type Task = Readonly<{
  schema_version: 1; reference: Readonly<{ workspace: Workspace; task_id: string }>;
  owner: Principal; title: string; definition: null; input_refs: readonly ArtifactReference[];
  status: "draft"; revision: number; created_at: string; updated_at: string;
}>;
export type Baseline = Readonly<{ task: Task; manifest: Manifest }>;
export type DraftGuard = Readonly<{ generation: number; revision: number; contextEpoch: number }>;
export type Allocation = Readonly<{ artifact_id: string; revision_id: string }>;
const encoder = new TextEncoder();

export const invalid = (): never => { throw new Error("taskSave.invalid"); };
export function record(value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return invalid();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return invalid();
  const own = Reflect.ownKeys(value), descriptors = Object.getOwnPropertyDescriptors(value);
  if (own.length !== keys.length || own.some(key => typeof key !== "string" || !keys.includes(key))
    || Object.values(descriptors).some(descriptor => !("value" in descriptor))) return invalid();
  return value as Record<string, unknown>;
}
export function list(value: unknown): readonly unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype
    || value.length > 32 || Reflect.ownKeys(value).length !== value.length + 1) return invalid();
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (let index = 0; index < value.length; index++) {
    if (!descriptors[index] || !("value" in descriptors[index])) return invalid();
  }
  return value;
}
export function integer(value: unknown, min = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min) return invalid();
  return value;
}
export function uuid(value: unknown): string {
  if (typeof value !== "string" || value.length !== 36
    || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
    || value === "00000000-0000-0000-0000-000000000000") return invalid();
  return value;
}
function text(value: unknown, maxBytes: number): string {
  if (typeof value !== "string" || /[\uD800-\uDFFF]/u.test(value)
    || /[\u0000-\u001f\u007f-\u009f]/.test(value)
    || value.length > maxBytes || encoder.encode(value).length > maxBytes) return invalid();
  return value;
}
export function title(value: unknown): string {
  const result = text(value, 256);
  if (!/[^\p{White_Space}]/u.test(result)) return invalid();
  return result;
}
function timestamp(value: unknown): string {
  if (typeof value !== "string"
    || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?(?:Z|\+00:00)$/.test(value)
    || !Number.isFinite(Date.parse(value))
    || new Date(value).toISOString().slice(0, 19) !== value.slice(0, 19)) return invalid();
  return value;
}
export function same(left: unknown, right: unknown): boolean {
  // Only reconstructed, frozen data reaches here; no caller toJSON or property getters.
  return JSON.stringify(left) === JSON.stringify(right);
}
export function guard(value: unknown): DraftGuard {
  const input = record(value, ["generation", "revision", "contextEpoch"]);
  return Object.freeze({ generation: integer(input.generation), revision: integer(input.revision),
    contextEpoch: integer(input.contextEpoch) });
}
export function allocation(value: unknown): Allocation {
  const input = record(value, ["artifact_id", "revision_id"]);
  return Object.freeze({ artifact_id: uuid(input.artifact_id), revision_id: uuid(input.revision_id) });
}
function workspace(value: unknown): Workspace {
  const input = record(value, ["authority_id", "workspace_id"]);
  return Object.freeze({ authority_id: uuid(input.authority_id), workspace_id: uuid(input.workspace_id) });
}
function principal(value: unknown): Principal {
  const input = record(value, ["authority_id", "principal_id"]);
  return Object.freeze({ authority_id: uuid(input.authority_id), principal_id: uuid(input.principal_id) });
}
export function reference(value: unknown): ArtifactReference {
  const input = record(value, ["workspace", "artifact_id", "revision_id", "sha256"]);
  if (typeof input.sha256 !== "string" || input.sha256.length !== 64 || /[^a-f0-9]/.test(input.sha256)) return invalid();
  return Object.freeze({ workspace: workspace(input.workspace), artifact_id: uuid(input.artifact_id),
    revision_id: uuid(input.revision_id), sha256: input.sha256 });
}
export function manifest(value: unknown): Manifest {
  const input = record(value, ["schema_version", "title", "payload"]);
  if (input.schema_version !== 1) return invalid();
  const label = title(input.title), seen = new Set<string>();
  const payload = list(input.payload).map(value => {
    const item = record(value, ["kind", "filename", "language", "artifact"]);
    if (item.kind !== "text" || !isTaskPayloadLanguage(item.language)) return invalid();
    const filename = text(item.filename, 128 * 4);
    if (!filename.length || filename.length > 128 || filename === "." || filename === ".."
      || /[/\\]/.test(filename)) return invalid();
    const artifact = reference(item.artifact), key = `${artifact.artifact_id}:${artifact.revision_id}`;
    if (seen.has(key)) return invalid();
    seen.add(key);
    return Object.freeze({ kind: "text" as const, filename, language: item.language, artifact });
  });
  if (payload.some(item => !same(item.artifact.workspace, payload[0].artifact.workspace))) return invalid();
  const result = Object.freeze({ schema_version: 1 as const, title: label, payload: Object.freeze(payload) });
  if (encoder.encode(JSON.stringify(result)).length > 64 * 1024) return invalid();
  return result;
}
function task(value: unknown): Task {
  const input = record(value, ["schema_version", "reference", "owner", "title", "definition", "input_refs",
    "status", "revision", "created_at", "updated_at"]);
  if (input.schema_version !== 1 || input.definition !== null || input.status !== "draft") return invalid();
  const target = record(input.reference, ["workspace", "task_id"]);
  const selected = Object.freeze({ workspace: workspace(target.workspace), task_id: uuid(target.task_id) });
  const owner = principal(input.owner);
  if (owner.authority_id !== selected.workspace.authority_id) return invalid();
  return Object.freeze({ schema_version: 1, reference: selected, owner, title: title(input.title),
    definition: null, input_refs: Object.freeze(list(input.input_refs).map(reference)), status: "draft",
    revision: integer(input.revision, 1), created_at: timestamp(input.created_at), updated_at: timestamp(input.updated_at) });
}
export function baseline(value: unknown): Baseline {
  const input = record(value, ["task", "manifest"]), selected = task(input.task), content = manifest(input.manifest);
  if (selected.title !== content.title || !same(selected.input_refs, content.payload.map(item => item.artifact))
    || content.payload.some(item => !same(item.artifact.workspace, selected.reference.workspace))) return invalid();
  return Object.freeze({ task: selected, manifest: content });
}
export function publicationResponse(value: unknown, expected: Readonly<{
  reference: ArtifactReference; owner: Principal; title: string; byte_length: number;
}>): ArtifactReference {
  const input = record(value, ["schema_version", "artifact"]);
  if (input.schema_version !== 1) return invalid();
  const artifact = record(input.artifact, ["schema_version", "reference", "owner", "sequence", "parent",
    "kind", "title", "media_type", "byte_length", "created_at"]);
  const pin = reference(artifact.reference), owner = principal(artifact.owner);
  timestamp(artifact.created_at);
  if (artifact.schema_version !== 1 || artifact.sequence !== 1 || artifact.parent !== null
    || artifact.kind !== "cyanrex.task-text" || artifact.media_type !== "text/plain"
    || artifact.title !== expected.title || artifact.byte_length !== expected.byte_length
    || !same(pin, expected.reference) || !same(owner, expected.owner)) return invalid();
  return pin;
}
export function replacementResponse(value: unknown, original: Task, expected: Manifest): void {
  const input = record(value, ["schema_version", "task", "manifest"]);
  if (input.schema_version !== 1) return invalid();
  const result = baseline({ task: input.task, manifest: input.manifest });
  if (!same(result.manifest, expected) || !same(result.task.reference, original.reference)
    || !same(result.task.owner, original.owner) || result.task.revision !== original.revision + 1
    || result.task.created_at !== original.created_at) return invalid();
}
