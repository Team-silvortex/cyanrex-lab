export type CFunctionSymbol = {
  name: string;
  line: number;
};

export type CSectionSymbol = {
  name: string;
  line: number;
};

export type CMetadata = {
  includes: string[];
  injectedIncludes: string[];
  functions: CFunctionSymbol[];
  sections: CSectionSymbol[];
  hasGplLicense: boolean;
  bytes: number;
  lines: number;
};

export type DiagnosticSeverity = "error" | "warning" | "info";

export type CDiagnostic = {
  line: number;
  column: number;
  endColumn: number;
  severity: DiagnosticSeverity;
  message: string;
};

export type AnalysisResult = {
  metadata: CMetadata;
  diagnostics: CDiagnostic[];
};

export function analyzeCCode(
  code: string,
  injectedIncludes: string[] = [],
): AnalysisResult {
  const lines = code.split(/\r?\n/);
  const includes = Array.from(code.matchAll(/^\s*#include\s*[<"]([^>"]+)[>"]/gm)).map(
    (m) => m[1],
  );

  const functions: CFunctionSymbol[] = [];
  const functionRegex =
    /^\s*(?:static\s+)?(?:__always_inline\s+)?[A-Za-z_][\w\s\*]*\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{/;
  lines.forEach((line, idx) => {
    const match = line.match(functionRegex);
    if (match) {
      functions.push({ name: match[1], line: idx + 1 });
    }
  });

  const sections = Array.from(code.matchAll(/SEC\("([^"]+)"\)/g)).map((m) => ({
    name: m[1],
    line: code.slice(0, m.index ?? 0).split(/\r?\n/).length,
  }));

  const hasGplLicense = /_license\[\]\s*SEC\("license"\)\s*=\s*"GPL"/.test(code);

  const diagnostics: CDiagnostic[] = [];

  const mergedIncludes = new Set([...includes, ...injectedIncludes]);

  const hasVmlinux = mergedIncludes.has("vmlinux.h");

  if (!hasVmlinux && !mergedIncludes.has("linux/bpf.h")) {
    diagnostics.push({
      line: 1,
      column: 1,
      endColumn: 1,
      severity: "warning",
      message: "Missing #include <linux/bpf.h> (or use #include <vmlinux.h> for CO-RE)",
    });
  }

  if (!mergedIncludes.has("bpf/bpf_helpers.h")) {
    diagnostics.push({
      line: 1,
      column: 1,
      endColumn: 1,
      severity: "warning",
      message: "Missing #include <bpf/bpf_helpers.h>",
    });
  }

  const hasTypedSchedCtx = /\btrace_event_raw_sched_switch\b/.test(code);
  if (hasTypedSchedCtx && !hasVmlinux) {
    diagnostics.push({
      line: 1,
      column: 1,
      endColumn: 1,
      severity: "warning",
      message: "Typed tracepoint ctx requires #include <vmlinux.h>",
    });
  }

  const hasRingbufUsage = /\bbpf_ringbuf_(reserve|submit|discard)\b/.test(code);
  if (hasRingbufUsage && !mergedIncludes.has("bpf/bpf_tracing.h")) {
    diagnostics.push({
      line: 1,
      column: 1,
      endColumn: 1,
      severity: "warning",
      message: "Ringbuf tracing helpers typically require #include <bpf/bpf_tracing.h>",
    });
  }

  if (sections.length === 0) {
    diagnostics.push({
      line: 1,
      column: 1,
      endColumn: 1,
      severity: "error",
      message: "No SEC(\"...\") section found",
    });
  }

  if (!hasGplLicense) {
    diagnostics.push({
      line: Math.max(lines.length, 1),
      column: 1,
      endColumn: 1,
      severity: "warning",
      message: "Missing GPL license declaration",
    });
  }

  const unsafeFnRegex = /\b(strcpy|strcat|gets|sprintf)\b/;
  lines.forEach((line, idx) => {
    const unsafeMatch = line.match(unsafeFnRegex);
    if (unsafeMatch) {
      const column = (unsafeMatch.index ?? 0) + 1;
      diagnostics.push({
        line: idx + 1,
        column,
        endColumn: column + unsafeMatch[0].length,
        severity: "warning",
        message: `Potentially unsafe function usage: ${unsafeMatch[0]}`,
      });
    }

    if (line.length > 180) {
      diagnostics.push({
        line: idx + 1,
        column: 181,
        endColumn: line.length + 1,
        severity: "info",
        message: "Line is very long; consider splitting for readability",
      });
    }
  });

  const braceBalance = countBraceBalance(code);
  if (braceBalance !== 0) {
    diagnostics.push({
      line: Math.max(lines.length, 1),
      column: 1,
      endColumn: 1,
      severity: "error",
      message: braceBalance > 0 ? "Unclosed '{' detected" : "Unmatched '}' detected",
    });
  }

  return {
    metadata: {
      includes,
      injectedIncludes,
      functions,
      sections,
      hasGplLicense,
      bytes: new TextEncoder().encode(code).length,
      lines: lines.length,
    },
    diagnostics,
  };
}

function countBraceBalance(code: string): number {
  let balance = 0;
  for (const char of code) {
    if (char === "{") balance += 1;
    if (char === "}") balance -= 1;
  }
  return balance;
}
