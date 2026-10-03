import { getEditorLanguage } from "../editor/languages";

export const MAX_TASK_PAYLOAD_ITEMS = 32;
export const MAX_TASK_DRAFT_BYTES = 8 * 1024 * 1024;
const MAX_TEXT_BYTES = 256 * 1024;
const encoder = new TextEncoder();

/** Local content IDs and revisions are editing guards, never server identities or permissions. */
export type TextPayloadItem = Readonly<{
  id: string;
  revision: number;
  kind: "text";
  filename: string;
  language: string;
  text: string;
}>;
/** Portable, local-only draft. This is not a TaskSnapshot or an ArtifactRef envelope. */
export type TaskDraft = Readonly<{
  format: "cyanrex.task-draft";
  version: 1;
  title: string;
  payload: readonly TextPayloadItem[];
}>;
type PayloadPatch = Partial<Pick<TextPayloadItem, "filename" | "language" | "text">>;
type ErrorKey = "invalid" | "stale" | "tooLarge" | "tooMany";
const fail = (key: ErrorKey = "invalid"): never => { throw new Error(`taskDraft.${key}`); };
const draftKeys = ["format", "version", "title", "payload"];
const itemKeys = ["id", "revision", "kind", "filename", "language", "text"];
const patchKeys = ["filename", "language", "text"];

function record(value: unknown, allowed: readonly string[], partial = false): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return fail();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return fail();
  const keys = Reflect.ownKeys(value);
  if ((!partial && keys.length !== allowed.length)
    || keys.some(key => typeof key !== "string" || !allowed.includes(key))) return fail();
  const descriptors = Object.getOwnPropertyDescriptors(value);
  // JSON has data properties only: helpers must not execute getters or custom toJSON methods.
  if (Object.values(descriptors).some(descriptor => !("value" in descriptor))) return fail();
  return value as Record<string, unknown>;
}

function localId(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9_-]{1,80}$/.test(value)) return fail();
  return value;
}

function revision(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) return fail();
  return value;
}

function text(value: unknown, maxBytes: number, multiline = false): string {
  if (typeof value !== "string") return fail();
  // Unicode mode matches lone surrogates, but not valid supplementary code points.
  if (/[\uD800-\uDFFF]/u.test(value)
    || (multiline ? /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/
      : /[\u0000-\u001f\u007f-\u009f]/).test(value)) return fail();
  if (value.length > maxBytes || encoder.encode(value).byteLength > maxBytes) return fail("tooLarge");
  return value;
}

function validateItem(value: unknown): TextPayloadItem {
  const source = record(value, itemKeys);
  const id = localId(source.id), version = revision(source.revision);
  if (source.kind !== "text" || typeof source.language !== "string" || !getEditorLanguage(source.language)) return fail();
  if (typeof source.filename !== "string" || source.filename.length < 1 || source.filename.length > 128
    || source.filename === "." || source.filename === ".." || /[/\\]/.test(source.filename)) return fail();
  const filename = text(source.filename, 128 * 4);
  return Object.freeze({ id, revision: version, kind: "text", filename,
    language: source.language, text: text(source.text, MAX_TEXT_BYTES, true) });
}

function validateDraft(value: unknown): TaskDraft {
  const source = record(value, draftKeys);
  if (source.format !== "cyanrex.task-draft" || source.version !== 1 || !Array.isArray(source.payload)) return fail();
  if (source.payload.length > MAX_TASK_PAYLOAD_ITEMS) return fail("tooMany");
  const entries = Object.getOwnPropertyDescriptors(source.payload);
  if (Object.getPrototypeOf(source.payload) !== Array.prototype
    || Reflect.ownKeys(source.payload).length !== source.payload.length + 1) return fail();
  const title = text(source.title, 256);
  const seen = new Set<string>();
  const payload: TextPayloadItem[] = [];
  for (let index = 0; index < source.payload.length; index++) {
    // Indexed data descriptors exclude holes, accessors and a caller's custom iterator.
    const entry = entries[String(index)];
    if (!entry || !("value" in entry)) return fail();
    const item = validateItem(entry.value);
    if (seen.has(item.id)) return fail();
    seen.add(item.id); payload.push(item);
  }
  return Object.freeze({ format: "cyanrex.task-draft", version: 1, title, payload: Object.freeze(payload) });
}

export function createTaskDraft(): TaskDraft {
  return validateDraft({ format: "cyanrex.task-draft", version: 1, title: "", payload: [] });
}

export function createTextPayloadItem(id: string): TextPayloadItem {
  return validateItem({ id, revision: 1, kind: "text", filename: "untitled.txt", language: "plaintext", text: "" });
}

export function updateTaskTitle(draft: TaskDraft, title: string): TaskDraft {
  return validateDraft({ ...validateDraft(draft), title });
}

export function addPayloadItem(draft: TaskDraft, item: TextPayloadItem): TaskDraft {
  const current = validateDraft(draft);
  return validateDraft({ ...current, payload: [...current.payload, item] });
}

function target(draft: TaskDraft, id: string, expectedRevision: number): number {
  localId(id); revision(expectedRevision);
  const index = draft.payload.findIndex(item => item.id === id);
  if (index < 0) return fail();
  if (draft.payload[index].revision !== expectedRevision) return fail("stale");
  return index;
}

export function updatePayloadItem(
  draft: TaskDraft, id: string, expectedRevision: number, patch: PayloadPatch,
): TaskDraft {
  const current = validateDraft(draft), index = target(current, id, expectedRevision);
  const changes = record(patch, patchKeys, true);
  if (expectedRevision === Number.MAX_SAFE_INTEGER) return fail();
  return validateDraft({ ...current, payload: current.payload.map((item, position) => position === index
    ? { ...item, ...changes, revision: expectedRevision + 1 } : item) });
}

export function removePayloadItem(draft: TaskDraft, id: string, expectedRevision: number): TaskDraft {
  const current = validateDraft(draft), index = target(current, id, expectedRevision);
  return validateDraft({ ...current, payload: current.payload.filter((_, position) => position !== index) });
}

export function serializeTaskDraft(draft: TaskDraft): string {
  const serialized = JSON.stringify(validateDraft(draft));
  // Escaping and metadata count too; raw text fitting is not a promise that JSON will fit.
  if (encoder.encode(serialized).byteLength > MAX_TASK_DRAFT_BYTES) return fail("tooLarge");
  return serialized;
}

export function parseTaskDraft(bytes: Uint8Array): TaskDraft {
  if (!(bytes instanceof Uint8Array)) return fail();
  if (bytes.byteLength > MAX_TASK_DRAFT_BYTES) return fail("tooLarge");
  let value: unknown;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { return fail(); }
  return validateDraft(value);
}
