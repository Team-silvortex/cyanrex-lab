export type EditorCapability = "completion" | "hover" | "signature" | "definition"
  | "references" | "rename" | "symbols" | "format" | "diagnostics";
export type EditorLanguage = Readonly<{
  id: string;
  label: string;
  extension: string;
  tier: "semantic" | "structured" | "basic" | "text";
  capabilities: readonly EditorCapability[];
}>;

function language(
  id: string, label: string, extension: string, tier: EditorLanguage["tier"],
  capabilities: readonly EditorCapability[],
): EditorLanguage {
  return Object.freeze({ id, label, extension, tier, capabilities: Object.freeze([...capabilities]) });
}
const semantic: readonly EditorCapability[] = [
  "completion", "hover", "signature", "definition", "references", "rename", "symbols", "format", "diagnostics",
];

// Capabilities describe configured local providers, not external LSPs or installed toolchains.
// HTML's Monaco 0.55.1 worker has no validation implementation despite its mode option.
export const BUILTIN_LANGUAGES: readonly EditorLanguage[] = Object.freeze([
  language("typescript", "TypeScript", ".ts", "semantic", semantic),
  language("javascript", "JavaScript", ".js", "semantic", semantic),
  language("json", "JSON", ".json", "structured", ["completion", "hover", "symbols", "format", "diagnostics"]),
  language("html", "HTML", ".html", "structured", ["completion", "hover", "rename", "symbols", "format"]),
  language("css", "CSS", ".css", "structured", ["completion", "hover", "definition", "references", "rename", "symbols", "format", "diagnostics"]),
  language("rust", "Rust", ".rs", "basic", ["completion"]),
  language("python", "Python", ".py", "basic", ["completion"]),
  language("c", "C", ".c", "basic", ["completion"]),
  language("cpp", "C++", ".cpp", "basic", ["completion"]),
  language("markdown", "Markdown", ".md", "basic", ["completion"]),
  language("yaml", "YAML", ".yaml", "basic", ["completion"]),
  language("sql", "SQL", ".sql", "basic", ["completion"]),
  language("shell", "Shell", ".sh", "basic", ["completion"]),
  language("plaintext", "Plain text", ".txt", "text", []),
]);

export function getEditorLanguage(id: string): EditorLanguage | undefined {
  return BUILTIN_LANGUAGES.find(item => item.id === id);
}

const extensions = new Map<string, string>([
  ...BUILTIN_LANGUAGES.map(item => [item.extension, item.id] as [string, string]),
  // The scratch models use .ts/.js; JSX/TSX need extension-preserving models and are not claimed.
  [".mts", "typescript"], [".cts", "typescript"],
  [".mjs", "javascript"], [".cjs", "javascript"],
  [".htm", "html"], [".h", "c"], [".cc", "cpp"], [".cxx", "cpp"],
  [".hpp", "cpp"], [".hh", "cpp"], [".hxx", "cpp"],
  [".markdown", "markdown"], [".mdown", "markdown"], [".yml", "yaml"],
  [".bash", "shell"], [".zsh", "shell"],
]);
const shellNames = new Set([".bashrc", ".bash_profile", ".zshrc", ".profile"]);

export function detectEditorLanguage(filename: string): EditorLanguage {
  const basename = filename.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  const suffix = basename.slice(basename.lastIndexOf("."));
  const id = shellNames.has(basename) ? "shell" : extensions.get(suffix);
  return getEditorLanguage(id ?? "plaintext")!;
}
