import { runtimeTextRows } from "./locales/runtimeText";
import { cIntelligenceRows } from "./locales/cIntelligence";
export type Translate = (key: string, variables?: Record<string, string | number>) => string;
/** Presentation only. Unknown text, code and external diagnostics must retain their original bytes. */
const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const patterns = [...runtimeTextRows, ...cIntelligenceRows].map(([key, english]) => {
  const names: string[] = [];
  let offset = 0, pattern = "^";
  for (const match of english.matchAll(/\{(\w+)\}/g)) {
    pattern += escape(english.slice(offset, match.index)) + "([^\\r\\n]{1,512})";
    names.push(match[1]); offset = match.index! + match[0].length;
  }
  pattern += escape(english.slice(offset)) + "(?![\\s\\S])";
  return { key, names, pattern: new RegExp(pattern) };
});
export function localizeLegacyText(message: string, translate: Translate): string {
  if (message.length > 4096) return message;
  for (const entry of patterns) {
    const match = entry.pattern.exec(message);
    if (!match) continue;
    const variables = Object.fromEntries(entry.names.map((name, index) => [name, match[index + 1]]));
    if ((entry.key === "pathExists" || entry.key === "pathMissing")
      && !["/sys/kernel/btf/vmlinux", "/sys/fs/bpf"].includes(variables.path)) continue;
    return translate(`runtimeText.${entry.key}`, variables);
  }
  return message;
}
export function localDiagnosticCode(message: string): string | undefined {
  if (message === "Missing GPL license declaration") return "cyanrex.license.gpl";
  if (message === "Missing #include <bpf/bpf_helpers.h>") return "cyanrex.include.bpf/bpf_helpers.h";
  if (message === "Missing #include <linux/bpf.h> (or use #include <vmlinux.h> for CO-RE)") return "cyanrex.include.linux/bpf.h";
  return undefined;
}
