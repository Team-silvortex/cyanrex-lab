/** Inert portable metadata, independent of installed editors, providers and task policy. */
export function isTaskPayloadLanguage(value: unknown): value is string {
  // Use an exclusion check rather than `$`, which also matches before a final newline in JS.
  return typeof value === "string" && value.length >= 1 && value.length <= 64
    && /^[a-z]/.test(value) && !/[^a-z0-9_+.-]/.test(value);
}
