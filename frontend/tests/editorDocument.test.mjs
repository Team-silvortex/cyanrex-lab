import assert from "node:assert/strict";
import test from "node:test";
import { decodeEditorBytes, MAX_EDITOR_IMPORT_BYTES, safeEditorFilename } from "../src/features/editor/document.ts";

test("editor imports accept bounded UTF-8 text without changing its source", () => {
  const text = "// 中文\nconst café = '☕';\r\n";
  assert.equal(decodeEditorBytes(new TextEncoder().encode(text)), text);
  assert.equal(decodeEditorBytes(new Uint8Array()), "");
  assert.equal(decodeEditorBytes(new Uint8Array(MAX_EDITOR_IMPORT_BYTES).fill(32)).length, MAX_EDITOR_IMPORT_BYTES);
});

test("editor imports reject oversized bytes before decoding", () => {
  assert.equal(MAX_EDITOR_IMPORT_BYTES, 256 * 1024);
  assert.throws(() => decodeEditorBytes(new Uint8Array(MAX_EDITOR_IMPORT_BYTES + 1)), /tooLarge/);
});

test("editor imports do not silently replace malformed UTF-8 or accept binary controls", () => {
  for (const value of [[0xc3, 0x28], [0xff], [65, 0, 66], [27, 91, 65]]) {
    assert.throws(() => decodeEditorBytes(new Uint8Array(value)), /invalidText/);
  }
  assert.equal(decodeEditorBytes(new Uint8Array([0xef, 0xbb, 0xbf, 65])), "A");
});

test("download names are basenames, not paths or injected control text", () => {
  assert.equal(safeEditorFilename("../../main.rs"), "main.rs");
  assert.equal(safeEditorFilename("C:\\tmp\\draft.py"), "draft.py");
  assert.equal(safeEditorFilename("note\n\u202e.html"), "note__.html");
  assert.equal(safeEditorFilename("draft?.json"), "draft_.json");
  assert.equal(safeEditorFilename("中文.ts"), "中文.ts");
});

test("empty or reserved names have a safe download fallback", () => {
  for (const value of ["", "  ", ".", "..", "CON", "nul.txt", "LPT1.json"]) {
    assert.equal(safeEditorFilename(value), "untitled.txt");
  }
  assert.ok(safeEditorFilename("a".repeat(1000)).length <= 128);
});
