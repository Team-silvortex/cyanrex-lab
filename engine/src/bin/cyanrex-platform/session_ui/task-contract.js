// Pure supplied-wire agreement, not authentication, a reusable grant or commit provenance.
// No network, Cookies, storage, rendering, dispatch, publication or recovery is performed here.
const encoder = new TextEncoder();
const INDEX_BYTES = 128 * 1024, CONTENT_BYTES = 50 * 1024 * 1024;
const MAX_TEXT = 256 * 1024, MAX_ITEMS = 32;
const statuses = new Set(["draft", "ready", "in_progress", "blocked", "in_review", "cancelled"]);
const invalid = () => { throw new Error("taskRead.invalid"); };
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);

// Validate lexical structure BEFORE JSON.parse can erase duplicate keys or round unsafe integers.
// Keys must be canonical literal ASCII names; escaped string VALUES remain ordinary JSON data.
function decode(raw, budget) {
  if (typeof raw !== "string" || raw.length > budget || encoder.encode(raw).length > budget) invalid();
  let offset = 0, tokens = 0;
  const white = () => {
    while ([32, 9, 10, 13].includes(raw.charCodeAt(offset))) offset++;
  };
  const string = key => {
    const start = offset++;
    while (offset < raw.length) {
      const code = raw.charCodeAt(offset++);
      if (code === 34) return key ? raw.slice(start + 1, offset - 1) : undefined;
      if (code < 32) invalid();
      if (code === 92) {
        if (key) invalid();
        const escape = raw[offset++];
        if (escape === "u") {
          if (!/^[a-fA-F0-9]{4}$/.test(raw.slice(offset, offset + 4))) invalid();
          offset += 4;
        } else if (!escape || !'"\\/bfnrt'.includes(escape)) invalid();
      }
    }
    invalid();
  };
  const value = depth => {
    if (++tokens > 8192 || depth > 16) invalid();
    white();
    const first = raw[offset];
    if (first === '"') { string(false); return; }
    if (first === "{") {
      offset++; white();
      const keys = new Set();
      if (raw[offset] === "}") { offset++; return; }
      while (true) {
        white(); if (raw[offset] !== '"' || ++tokens > 8192) invalid();
        const key = string(true);
        if (keys.has(key)) invalid(); keys.add(key);
        white(); if (raw[offset++] !== ":") invalid();
        value(depth + 1); white();
        const end = raw[offset++]; if (end === "}") return;
        if (end !== ",") invalid();
      }
    }
    if (first === "[") {
      offset++; white(); let count = 0;
      if (raw[offset] === "]") { offset++; return; }
      while (true) {
        if (++count > MAX_ITEMS) invalid();
        value(depth + 1); white();
        const end = raw[offset++]; if (end === "]") return;
        if (end !== ",") invalid();
      }
    }
    for (const literal of ["true", "false", "null"]) {
      if (raw.startsWith(literal, offset)) { offset += literal.length; return; }
    }
    // All numeric fields in these exact DTOs are unsigned JSON-safe integer tokens.
    const start = offset;
    while (raw[offset] >= "0" && raw[offset] <= "9") {
      if (++offset - start > 16) invalid();
    }
    if (offset === start || (offset - start > 1 && raw[start] === "0")
      || !Number.isSafeInteger(Number(raw.slice(start, offset)))) invalid();
  };
  value(0); white(); if (offset !== raw.length) invalid();
  try { return JSON.parse(raw); } catch { return invalid(); }
}

function record(value, keys, partial = false) {
  if (!value || typeof value !== "object" || Array.isArray(value)) invalid();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) invalid();
  const own = Reflect.ownKeys(value), descriptors = Object.getOwnPropertyDescriptors(value);
  if ((!partial && own.length !== keys.length)
    || own.some(key => typeof key !== "string" || !keys.includes(key))
    || Object.values(descriptors).some(property => !("value" in property))) invalid();
  return value;
}
function list(value) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype
    || value.length > MAX_ITEMS || Reflect.ownKeys(value).length !== value.length + 1) invalid();
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (let index = 0; index < value.length; index++) {
    if (!descriptors[index] || !("value" in descriptors[index])) invalid();
  }
  return value;
}
function integer(value, min = 0, max = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < min || value > max) invalid();
  return value;
}
function uuid(value) {
  if (typeof value !== "string" || value.length !== 36
    || !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(value)
    || value === "00000000-0000-0000-0000-000000000000") invalid();
  return value;
}
function text(value, budget, multiline = false) {
  if (typeof value !== "string" || /[\uD800-\uDFFF]/u.test(value)
    || (multiline ? /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/
      : /[\u0000-\u001f\u007f-\u009f]/).test(value)
    || value.length > budget || encoder.encode(value).length > budget) invalid();
  return value;
}
function title(value) {
  const result = text(value, 256);
  if (!/[^\p{White_Space}]/u.test(result)) invalid();
  return result;
}
function timestamp(value) {
  if (typeof value !== "string"
    || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?(?:Z|\+00:00)$/.test(value)
    || !Number.isFinite(Date.parse(value))
    || new Date(value).toISOString().slice(0, 19) !== value.slice(0, 19)) invalid();
  return value;
}
function workspace(value) {
  const input = record(value, ["authority_id", "workspace_id"]);
  return Object.freeze({ authority_id: uuid(input.authority_id), workspace_id: uuid(input.workspace_id) });
}
function taskRef(value) {
  const input = record(value, ["workspace", "task_id"]);
  return Object.freeze({ workspace: workspace(input.workspace), task_id: uuid(input.task_id) });
}
function artifactRef(value) {
  const input = record(value, ["workspace", "artifact_id", "revision_id", "sha256"]);
  if (typeof input.sha256 !== "string" || input.sha256.length !== 64 || /[^a-f0-9]/.test(input.sha256)) invalid();
  return Object.freeze({ workspace: workspace(input.workspace), artifact_id: uuid(input.artifact_id),
    revision_id: uuid(input.revision_id), sha256: input.sha256 });
}
function status(value) { if (!statuses.has(value)) invalid(); return value; }
function indexEntry(value) {
  const input = record(value, ["reference", "revision", "title", "status", "definition_ref", "updated_at", "payload_items"]);
  // This retained host is manual-only; catalogue records are not implicitly admitted.
  if (input.definition_ref !== null) invalid();
  return Object.freeze({ reference: taskRef(input.reference), revision: integer(input.revision, 1),
    title: title(input.title), status: status(input.status), definition_ref: null,
    updated_at: timestamp(input.updated_at), payload_items: integer(input.payload_items, 0, MAX_ITEMS) });
}

/** Current metadata only. A cursor/scope here is structural data, never a resource access grant. */
export function parseIndex(raw, options = {}) {
  const config = record(options, ["after", "scope", "limit"], true);
  const after = config.after == null ? null : uuid(config.after);
  const limit = config.limit === undefined ? 20 : integer(config.limit, 1, MAX_ITEMS);
  let scope = config.scope == null ? null : workspace(config.scope);
  const input = record(decode(raw, INDEX_BYTES), ["schema_version", "entries", "next_after"]);
  if (input.schema_version !== 1) invalid();
  let previous = after;
  const entries = list(input.entries).map(value => {
    const entry = indexEntry(value), current = entry.reference;
    if (previous !== null && current.task_id <= previous) invalid();
    previous = current.task_id;
    if (scope === null) scope = current.workspace;
    if (!same(current.workspace, scope)) invalid();
    return entry;
  });
  if (entries.length > limit) invalid();
  const next = input.next_after === null ? null : taskRef(input.next_after);
  if (next !== null && (entries.length !== limit || !same(next, entries.at(-1)?.reference))) invalid();
  return Object.freeze({ schema_version: 1, entries: Object.freeze(entries), next_after: next });
}

function manifest(value, scope) {
  const input = record(value, ["schema_version", "title", "payload"]);
  if (input.schema_version !== 1) invalid();
  const seen = new Set();
  const payload = list(input.payload).map(value => {
    const item = record(value, ["kind", "filename", "language", "artifact"]);
    const filename = text(item.filename, 512), language = item.language;
    if (item.kind !== "text" || !filename.length || filename.length > 128 || filename === "." || filename === ".."
      || /[/\\]/.test(filename) || typeof language !== "string" || language.length > 64
      || !/^[a-z][a-z0-9_+.-]*$/.test(language)) invalid();
    const artifact = artifactRef(item.artifact), key = `${artifact.artifact_id}:${artifact.revision_id}`;
    if (!same(artifact.workspace, scope) || seen.has(key)) invalid(); seen.add(key);
    return Object.freeze({ kind: "text", filename, language, artifact });
  });
  return Object.freeze({ schema_version: 1, title: title(input.title), payload: Object.freeze(payload) });
}
function task(value) {
  const input = record(value, ["schema_version", "reference", "owner", "title", "definition", "input_refs",
    "status", "revision", "created_at", "updated_at"]);
  if (input.schema_version !== 1 || input.definition !== null) invalid();
  const reference = taskRef(input.reference), principal = record(input.owner, ["authority_id", "principal_id"]);
  const owner = Object.freeze({ authority_id: uuid(principal.authority_id), principal_id: uuid(principal.principal_id) });
  if (owner.authority_id !== reference.workspace.authority_id) invalid();
  return Object.freeze({ schema_version: 1, reference, owner, title: title(input.title), definition: null,
    input_refs: Object.freeze(list(input.input_refs).map(artifactRef)), status: status(input.status),
    revision: integer(input.revision, 1), created_at: timestamp(input.created_at), updated_at: timestamp(input.updated_at) });
}

/** One supplied read response; caller still owns current UI epoch checks around this async work. */
export async function parseContent(raw, selectedEntry, selectedScope = null) {
  // Snapshot all caller-selected metadata before hashing yields to any obsolete UI callback.
  const selected = indexEntry(selectedEntry), scope = selectedScope == null
    ? selected.reference.workspace : workspace(selectedScope);
  if (!same(scope, selected.reference.workspace)) invalid();
  const input = record(decode(raw, CONTENT_BYTES), ["schema_version", "task", "manifest", "contents"]);
  if (input.schema_version !== 1) invalid();
  const result = task(input.task), content = manifest(input.manifest, scope);
  if (!same(result.reference, selected.reference) || !same(result.reference.workspace, scope)
    || result.revision < selected.revision || result.title !== content.title
    || !same(result.input_refs, content.payload.map(item => item.artifact))) invalid();
  if (result.revision === selected.revision && (result.title !== selected.title || result.status !== selected.status
    || result.updated_at !== selected.updated_at || content.payload.length !== selected.payload_items)) invalid();
  const contents = list(input.contents).map((value, index) => {
    const item = record(value, ["artifact", "text"]), artifact = artifactRef(item.artifact);
    if (!same(artifact, content.payload[index]?.artifact)) invalid();
    return Object.freeze({ artifact, text: text(item.text, MAX_TEXT, true) });
  });
  if (contents.length !== content.payload.length) invalid();
  for (const item of contents) {
    const bytes = encoder.encode(item.text), hash = await crypto.subtle.digest("SHA-256", bytes);
    const digest = Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, "0")).join("");
    if (digest !== item.artifact.sha256) invalid();
  }
  return Object.freeze({ schema_version: 1, task: result, manifest: content, contents: Object.freeze(contents) });
}

// Metadata preparation only checks supplied baseline agreement. It neither rehashes those bytes
// nor establishes that the baseline was obtained from a current authorized server response.
function metadataBaseline(value) {
  const input = record(value, ["schema_version", "task", "manifest", "contents"]);
  if (input.schema_version !== 1) invalid();
  const baseline = task(input.task), content = manifest(input.manifest, baseline.reference.workspace);
  if (baseline.status !== "draft" || baseline.title !== content.title
    || !same(baseline.input_refs, content.payload.map(item => item.artifact))) invalid();
  const contents = list(input.contents);
  if (contents.length !== content.payload.length) invalid();
  contents.forEach((value, index) => {
    const item = record(value, ["artifact", "text"]);
    if (!same(artifactRef(item.artifact), content.payload[index].artifact)) invalid();
    text(item.text, MAX_TEXT, true);
  });
  return { task: baseline, manifest: content };
}

function metadataDescriptor(value, baseline) {
  const input = record(value, ["task_id", "body"]), body = record(input.body,
    ["schema_version", "expected_revision", "manifest"]);
  if (uuid(input.task_id) !== baseline.task.reference.task_id || body.schema_version !== 1
    || integer(body.expected_revision, 1, Number.MAX_SAFE_INTEGER - 1) !== baseline.task.revision) invalid();
  const content = manifest(body.manifest, baseline.task.reference.workspace);
  // This slice cannot add/remove/reorder pins or change immutable bytes, even in a supplied descriptor.
  if (!same(content.payload.map(item => item.artifact), baseline.task.input_refs)
    || same(content, baseline.manifest)) invalid();
  const result = Object.freeze({ schema_version: 1, expected_revision: body.expected_revision, manifest: content });
  if (encoder.encode(JSON.stringify(result)).length > 64 * 1024) invalid();
  return Object.freeze({ task_id: baseline.task.reference.task_id, body: result });
}

function metadataCheck(action) {
  try { return action(); } catch { throw new Error("taskMetadata.invalid"); }
}

/** Frozen one-PUT descriptor only. Caller owns explicit confirmation, dispatch and current UI epoch. */
export function prepareMetadataSave(snapshot, edits) {
  return metadataCheck(() => {
    const baseline = metadataBaseline(snapshot), input = record(edits, ["title", "items"]);
    const items = list(input.items);
    if (items.length !== baseline.manifest.payload.length) invalid();
    const payload = items.map((value, index) => {
      const item = record(value, ["filename", "language"]);
      return { kind: "text", filename: item.filename, language: item.language,
        artifact: baseline.manifest.payload[index].artifact };
    });
    return metadataDescriptor({ task_id: baseline.task.reference.task_id,
      body: { schema_version: 1, expected_revision: baseline.task.revision,
        manifest: { schema_version: 1, title: input.title, payload } } }, baseline);
  });
}

/** Supplied strict metadata receipt, not a content reread or COMMIT provenance. Caller requires HTTP 200. */
export function parseMetadataSaved(raw, snapshot, descriptor) {
  return metadataCheck(() => {
    const baseline = metadataBaseline(snapshot), expected = metadataDescriptor(descriptor, baseline);
    const input = record(decode(raw, INDEX_BYTES), ["schema_version", "task", "manifest"]);
    if (input.schema_version !== 1) invalid();
    const result = task(input.task), content = manifest(input.manifest, baseline.task.reference.workspace);
    if (!same(result.reference, baseline.task.reference) || !same(result.owner, baseline.task.owner)
      || result.created_at !== baseline.task.created_at || result.status !== "draft"
      || result.revision !== expected.body.expected_revision + 1
      || result.title !== content.title || !same(result.input_refs, baseline.task.input_refs)
      || !same(content, expected.body.manifest)) invalid();
    return Object.freeze({ schema_version: 1, task: result, manifest: content });
  });
}
