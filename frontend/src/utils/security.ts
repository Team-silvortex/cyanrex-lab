const HTML_ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
  "`": "&#96;",
};

export function sanitizeForDisplay(input: string): string {
  return input.replace(/[&<>"'`]/g, (char) => HTML_ESCAPE_MAP[char]);
}
