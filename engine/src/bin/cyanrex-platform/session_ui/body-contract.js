// Pure fixed-slot bridge. A supplied read baseline and receipt are not authorization or COMMIT provenance.
import { metadataBaseline, decodeSaveReceipt as decodeRawReceipt } from './task-contract.js';
import { prepareTaskBoundContentSave } from './task-save/taskContentSave.js';
import { record, list, invalid } from './task-save/taskSaveContract.js';

/** Apply a textarea edit without normalizing unchanged original line endings. */
export function editTextareaText(original, displayed) {
  if (typeof original !== 'string' || typeof displayed !== 'string') return invalid();
  const normalized = original.replace(/\r\n?/g, '\n');
  if (normalized === displayed) return original;
  let prefix = 0, suffix = 0;
  while (prefix < normalized.length && prefix < displayed.length
    && normalized[prefix] === displayed[prefix]) prefix++;
  while (suffix < normalized.length - prefix && suffix < displayed.length - prefix
    && normalized[normalized.length - suffix - 1] === displayed[displayed.length - suffix - 1]) suffix++;
  const originalOffset = normalizedOffset => {
    let offset = 0;
    for (let index = 0; index < normalizedOffset; index++, offset++) {
      if (original[offset] === '\r' && original[offset + 1] === '\n') offset++;
    }
    return offset;
  };
  const ending = original.match(/\r\n|\r|\n/)?.[0] || '\n';
  const changed = displayed.slice(prefix, displayed.length - suffix).replace(/\n/g, ending);
  return original.slice(0, originalOffset(prefix)) + changed
    + original.slice(originalOffset(normalized.length - suffix));
}

/** Own fixed local slots, then delegate all hashing, handoff and receipt agreement to the shared planner. */
export async function prepareBodySave(snapshot, edits, guard, allocations) {
  try {
    const baseline = metadataBaseline(snapshot), input = record(edits, ['title', 'items']);
    const items = list(input.items);
    if (items.length !== baseline.manifest.payload.length) return invalid();
    const payload = items.map((value, index) => {
      const item = record(value, ['filename', 'language', 'text']);
      return Object.freeze({ id: `slot-${index}`, revision: 1, kind: 'text',
        filename: item.filename, language: item.language, text: item.text });
    });
    const draft = Object.freeze({ format: 'cyanrex.task-draft', version: 1,
      title: input.title, payload: Object.freeze(payload) });
    // The shared factory snapshots the full draft, baseline, guard and allocations before its first await.
    return await prepareTaskBoundContentSave(draft, baseline, guard, allocations);
  } catch { return invalid(); }
}

function freezeReceipt(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freezeReceipt(child);
    Object.freeze(value);
  }
  return value;
}

/** Decode raw JSON before its semantics erase duplicate/escaped keys or unsafe numeric tokens. */
export function decodeSaveReceipt(raw) {
  try { return freezeReceipt(decodeRawReceipt(raw)); }
  catch { return invalid(); }
}
