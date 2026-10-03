export const MAX_EDITOR_IMPORT_BYTES = 256 * 1024;

/** Plain UTF-8 only. Never evaluate, upload or silently repair a file. */
export function decodeEditorBytes(bytes: Uint8Array): string {
  if (bytes.byteLength > MAX_EDITOR_IMPORT_BYTES) throw new Error("editor.tooLarge");
  let text: string;
  try { text = new TextDecoder("utf-8", { fatal: true }).decode(bytes); }
  catch { throw new Error("editor.invalidText"); }
  if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(text)) throw new Error("editor.invalidText");
  return text;
}

export function safeEditorFilename(name: string): string {
  const basename = (name.split(/[/\\]/).at(-1) || "")
    .replace(/[<>:"|?*\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g, "_")
    .trim().slice(0, 128).replace(/[. ]+$/, "");
  if (!basename || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(basename)) return "untitled.txt";
  return basename;
}
