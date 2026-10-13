import { decodeEditorBytes, safeEditorFilename } from "../editor/document";
import { detectEditorLanguage } from "../editor/languages";
import {
  addPayloadItem, createTextPayloadItem, MAX_TASK_PAYLOAD_ITEMS, parseTaskDraft,
  serializeTaskDraft, type TaskDraft,
} from "./taskDraft";

export const MAX_TEXT_FILE_BYTES = 256 * 1024;
export const TEXT_FILE_IMPORT_TIMEOUT_MS = 10_000;
const encoder = new TextEncoder();
type Selection = { name: string; size: number };
const fail = (key: "invalid" | "tooLarge" | "tooMany" | "readFailed" | "stale"): never => {
  throw new Error(`taskDraft.${key}`);
};

function metadata(draft: TaskDraft, files: readonly File[]): Selection[] {
  if (!Array.isArray(files) || files.length === 0) return fail("invalid");
  if (draft.payload.length + files.length > MAX_TASK_PAYLOAD_ITEMS) return fail("tooMany");
  return files.map(file => {
    if (!file) return fail("invalid");
    const suppliedName = file.name, size = file.size;
    if (typeof suppliedName !== "string" || !Number.isSafeInteger(size) || size < 0) {
      return fail("invalid");
    }
    if (size > MAX_TEXT_FILE_BYTES) return fail("tooLarge");
    const name = safeEditorFilename(suppliedName);
    // The display adapter truncates valid code points, but does not repair an existing lone surrogate.
    if (/[\uD800-\uDFFF]/u.test(name)) return fail("invalid");
    return { name, size };
  });
}

/** Local display metadata only; never filesystem access or a persisted Task identity. */
export function preflightTextFiles(draft: TaskDraft, files: readonly File[]): Selection[] {
  serializeTaskDraft(draft);
  return metadata(draft, files);
}

/** All-or-nothing local preparation. File.arrayBuffer itself cannot be cancelled or remotely uploaded. */
export async function importTextFiles(
  draft: TaskDraft, files: readonly File[], createId: () => string,
  signal: AbortSignal, checkReviewed: () => void,
): Promise<TaskDraft> {
  const deadline = Date.now() + TEXT_FILE_IMPORT_TIMEOUT_MS;
  // The core creates an immutable independent copy before any asynchronous read can run.
  const original = parseTaskDraft(encoder.encode(serializeTaskDraft(draft)));
  const reviewed = metadata(original, files);
  const selection = files.map((file, index) => ({ file, ...reviewed[index] }));
  let expired = false;
  const check = () => {
    if (signal.aborted) return fail("stale");
    try { checkReviewed(); } catch { return fail("stale"); }
    if (signal.aborted) return fail("stale");
    if (expired || Date.now() >= deadline) return fail("readFailed");
  };
  check();
  let stop: (error: Error) => void = () => {};
  const interruption = new Promise<never>((_resolve, reject) => { stop = reject; });
  // An abort during a synchronous review/ID callback can precede the next race attachment.
  // Handle that promise immediately; late read rejections are likewise owned by Promise.race.
  void interruption.catch(() => {});
  const abort = () => stop(new Error("taskDraft.stale"));
  const timer = setTimeout(() => {
    expired = true;
    stop(new Error("taskDraft.readFailed"));
  }, Math.max(0, deadline - Date.now()));
  try {
    signal.addEventListener("abort", abort, { once: true });
    check();
    const texts: string[] = [];
    for (const selected of selection) {
      check();
      let result: ArrayBuffer;
      try {
        result = await Promise.race([
          Promise.resolve().then(() => { check(); return selected.file.arrayBuffer(); }),
          interruption,
        ]);
      } catch {
        check();
        return fail("readFailed");
      }
      check();
      if (!(result instanceof ArrayBuffer) || result.byteLength !== selected.size) return fail("readFailed");
      let text: string;
      try { text = decodeEditorBytes(new Uint8Array(result)); }
      catch { check(); return fail("invalid"); }
      // Editor UTF-8 decoding rejects C0/DEL; the Task payload contract also rejects C1 controls.
      if (/[\u0080-\u009f]/.test(text)) return fail("invalid");
      check();
      texts.push(text);
    }
    let next = original;
    for (let index = 0; index < selection.length; index++) {
      check();
      let id: string;
      try { id = createId(); } catch { check(); return fail("invalid"); }
      check();
      const selected = selection[index];
      next = addPayloadItem(next, { ...createTextPayloadItem(id), filename: selected.name,
        language: detectEditorLanguage(selected.name).id, text: texts[index] });
    }
    check();
    // JSON escaping, every old item and metadata count toward the complete portable 8 MiB bound.
    serializeTaskDraft(next);
    check();
    return next;
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", abort);
  }
}
