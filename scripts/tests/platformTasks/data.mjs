import { createHash } from "node:crypto";

export const id = value => `00000000-0000-4000-8000-${value.toString(16).padStart(12, "0")}`;
export const workspace = { authority_id: id(1), workspace_id: id(2) };
export const owner = { authority_id: id(1), principal_id: id(3) };
export const time = "2026-10-26T01:02:03Z";
export const text = "讨论记录，不要求代码或 Run。😀\n<script>window.__taskExecuted=1</script>\n";
export const hash = value => createHash("sha256").update(value, "utf8").digest("hex");
export const copy = value => structuredClone(value);
export function entry(number = 10, patch = {}) {
  return { reference: { workspace: copy(workspace), task_id: id(number) }, revision: 5,
    title: `Discussion ${number}`, status: "draft", definition_ref: null,
    updated_at: time, payload_items: 2, ...patch };
}
export function page(entries = [entry()], next_after = null) {
  return { schema_version: 1, entries, next_after };
}
export function content(selected = entry(), texts = [text, "Second plain note\n"]) {
  const payload = texts.map((value, index) => ({ kind: "text", filename: `notes-${index}.txt`,
    language: index ? "plaintext" : "unknown-valid+note", artifact: { workspace: copy(workspace),
      artifact_id: id(100 + index), revision_id: id(200 + index), sha256: hash(value) } }));
  return { schema_version: 1, task: { schema_version: 1, reference: copy(selected.reference),
    owner: copy(owner), title: selected.title, definition: null,
    input_refs: payload.map(item => copy(item.artifact)), status: selected.status,
    revision: selected.revision, created_at: time, updated_at: selected.updated_at },
  manifest: { schema_version: 1, title: selected.title, payload },
  contents: payload.map((item, index) => ({ artifact: copy(item.artifact), text: texts[index] })) };
}
