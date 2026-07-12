import type * as Monaco from "monaco-editor";
import type { EbpfCompletionResponse } from "../features/ebpf/models";

type HoverDoc = {
  title: string;
  detail: string;
};

const HOVER_DOCS: Record<string, HoverDoc> = {
  SEC: {
    title: "SEC(\"...\")",
    detail: "Declare eBPF section. Common values: xdp, tc, kprobe/<func>, tracepoint/<cat>/<name>.",
  },
  xdp_md: {
    title: "struct xdp_md",
    detail: "XDP context. Provides packet pointers and metadata for XDP programs.",
  },
  bpf_printk: {
    title: "bpf_printk(fmt, ...)",
    detail: "Debug helper for trace output (performance cost, debug use only).",
  },
  bpf_map_lookup_elem: {
    title: "bpf_map_lookup_elem(map, key)",
    detail: "Return pointer to value or NULL.",
  },
  bpf_map_update_elem: {
    title: "bpf_map_update_elem(map, key, value, flags)",
    detail: "Insert or update map entry.",
  },
  bpf_ringbuf_reserve: {
    title: "bpf_ringbuf_reserve(map, size, flags)",
    detail: "Reserve a record in ring buffer map; returns NULL on pressure.",
  },
  bpf_ringbuf_submit: {
    title: "bpf_ringbuf_submit(data, flags)",
    detail: "Submit reserved ringbuf record to user space reader.",
  },
  bpf_ktime_get_ns: {
    title: "bpf_ktime_get_ns()",
    detail: "Monotonic kernel timestamp in nanoseconds.",
  },
  bpf_get_smp_processor_id: {
    title: "bpf_get_smp_processor_id()",
    detail: "Return current CPU id.",
  },
  bpf_get_current_pid_tgid: {
    title: "bpf_get_current_pid_tgid()",
    detail: "Returns TGID in the upper 32 bits and PID in the lower 32 bits.",
  },
  bpf_get_current_comm: {
    title: "bpf_get_current_comm(buf, size)",
    detail: "Copies the current task command name into a verifier-visible buffer.",
  },
  bpf_probe_read_kernel: {
    title: "bpf_probe_read_kernel(dst, size, src)",
    detail: "Safely reads kernel memory into an eBPF stack or map buffer.",
  },
  BPF_MAP_TYPE_HASH: {
    title: "BPF_MAP_TYPE_HASH",
    detail: "General key/value hash map shared by eBPF and user space.",
  },
  BPF_MAP_TYPE_RINGBUF: {
    title: "BPF_MAP_TYPE_RINGBUF",
    detail: "Ordered variable-length event transport from eBPF to user space.",
  },
  trace_event_raw_sched_switch: {
    title: "struct trace_event_raw_sched_switch",
    detail: "Tracepoint context from vmlinux.h; includes next_pid/prev_pid fields.",
  },
  next_pid: {
    title: "ctx->next_pid",
    detail: "PID of the task that will be scheduled in.",
  },
  XDP_PASS: {
    title: "XDP_PASS",
    detail: "Allow packet to continue through network stack.",
  },
  XDP_DROP: {
    title: "XDP_DROP",
    detail: "Drop packet immediately at XDP hook.",
  },
  XDP_TX: {
    title: "XDP_TX",
    detail: "Transmit packet back on ingress interface.",
  },
  XDP_REDIRECT: {
    title: "XDP_REDIRECT",
    detail: "Redirect packet to another interface / map target.",
  },
};

const COMPLETIONS = [
  {
    label: "hash map declaration",
    insertText:
      'struct {\n  __uint(type, BPF_MAP_TYPE_HASH);\n  __uint(max_entries, ${1:1024});\n  __type(key, ${2:__u32});\n  __type(value, ${3:__u64});\n} ${4:counts} SEC(".maps");',
    detail: "BTF-style hash map declaration",
    kind: "snippet",
  },
  {
    label: "ring buffer declaration",
    insertText:
      'struct {\n  __uint(type, BPF_MAP_TYPE_RINGBUF);\n  __uint(max_entries, ${1:256 * 1024});\n} ${2:events} SEC(".maps");',
    detail: "BTF-style ring buffer map declaration",
    kind: "snippet",
  },
  {
    label: "per-cpu array declaration",
    insertText:
      'struct {\n  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);\n  __uint(max_entries, ${1:1});\n  __type(key, __u32);\n  __type(value, ${2:__u64});\n} ${3:stats} SEC(".maps");',
    detail: "Low-contention per-CPU array map",
    kind: "snippet",
  },
  {
    label: "SEC xdp",
    insertText: 'SEC("xdp")\\nint ${1:xdp_handler}(struct xdp_md *ctx) {\\n  return XDP_PASS;\\n}',
    detail: "XDP section snippet",
    kind: "snippet",
  },
  {
    label: "SEC tc",
    insertText: 'SEC("tc")\\nint ${1:tc_handler}(struct __sk_buff *skb) {\\n  return 0;\\n}',
    detail: "TC section snippet",
    kind: "snippet",
  },
  {
    label: "SEC tracepoint sched_switch",
    insertText:
      'SEC("tracepoint/sched/sched_switch")\\nint ${1:on_sched_switch}(struct trace_event_raw_sched_switch *ctx) {\\n  return 0;\\n}',
    detail: "Typed tracepoint context snippet",
    kind: "snippet",
  },
  {
    label: "#include <vmlinux.h>",
    insertText: "#include <vmlinux.h>",
    detail: "CO-RE/BTF generated kernel type metadata header",
    kind: "snippet",
  },
  {
    label: "GPL license",
    insertText: 'char _license[] SEC("license") = "GPL";',
    detail: "Required by many helpers/program types",
    kind: "snippet",
  },
  {
    label: "bpf_printk",
    insertText: 'bpf_printk("${1:msg}: %d", ${2:value});',
    detail: "Debug print helper",
    kind: "function",
  },
  {
    label: "bpf_map_lookup_elem",
    insertText: "bpf_map_lookup_elem(&${1:map}, &${2:key})",
    detail: "Lookup map value",
    kind: "function",
  },
  {
    label: "bpf_map_update_elem",
    insertText: "bpf_map_update_elem(&${1:map}, &${2:key}, &${3:value}, ${4:0})",
    detail: "Update map value",
    kind: "function",
  },
  {
    label: "bpf_ringbuf_reserve",
    insertText: "bpf_ringbuf_reserve(&${1:events}, sizeof(${2:*evt}), 0)",
    detail: "Reserve ringbuf record",
    kind: "function",
  },
  {
    label: "bpf_ringbuf_submit",
    insertText: "bpf_ringbuf_submit(${1:evt}, 0)",
    detail: "Submit ringbuf record",
    kind: "function",
  },
  {
    label: "bpf_get_current_pid_tgid",
    insertText: "bpf_get_current_pid_tgid()",
    detail: "Current TGID in high 32 bits and PID in low 32 bits",
    kind: "function",
  },
  {
    label: "bpf_get_current_comm",
    insertText: "bpf_get_current_comm(&${1:comm}, sizeof(${1:comm}))",
    detail: "Copy current task command name",
    kind: "function",
  },
  {
    label: "bpf_probe_read_kernel",
    insertText: "bpf_probe_read_kernel(&${1:dst}, sizeof(${1:dst}), ${2:src})",
    detail: "Verifier-safe kernel memory read",
    kind: "function",
  },
  {
    label: "XDP_PASS",
    insertText: "XDP_PASS",
    detail: "XDP action: pass",
    kind: "constant",
  },
  {
    label: "XDP_DROP",
    insertText: "XDP_DROP",
    detail: "XDP action: drop",
    kind: "constant",
  },
  {
    label: "XDP_TX",
    insertText: "XDP_TX",
    detail: "XDP action: tx",
    kind: "constant",
  },
  {
    label: "XDP_REDIRECT",
    insertText: "XDP_REDIRECT",
    detail: "XDP action: redirect",
    kind: "constant",
  },
] as const;

function toCompletionKind(monaco: typeof Monaco, kind: (typeof COMPLETIONS)[number]["kind"]) {
  if (kind === "function") return monaco.languages.CompletionItemKind.Function;
  if (kind === "constant") return monaco.languages.CompletionItemKind.Constant;
  return monaco.languages.CompletionItemKind.Snippet;
}

export function registerEbpfIntelligence(
  monaco: typeof Monaco,
  engineUrl: string,
): Monaco.IDisposable {
  const completion = monaco.languages.registerCompletionItemProvider("c", {
    triggerCharacters: ["#", "_", "b", "X", ".", ">"],
    async provideCompletionItems(model, position, _context, token) {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      const suggestions: Monaco.languages.CompletionItem[] = COMPLETIONS.map((item) => ({
        label: item.label,
        kind: toCompletionKind(monaco, item.kind),
        insertText: item.insertText,
        insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
        detail: item.detail,
        range,
      }));

      try {
        const response = await fetch(`${engineUrl}/ebpf/complete`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({
            code: model.getValue(),
            line: position.lineNumber,
            column: position.column,
          }),
        });
        if (!response.ok || token.isCancellationRequested) return { suggestions };
        const semantic = (await response.json()) as EbpfCompletionResponse;
        for (const item of semantic.items) {
          suggestions.push({
            label: item.label,
            kind: toSemanticCompletionKind(monaco, item.kind),
            insertText: item.insert_text || item.label,
            detail: `clang · ${item.detail}`,
            sortText: `0-${item.label}`,
            range,
          });
        }
      } catch {
        // Local snippets remain available when semantic completion is offline.
      }
      return { suggestions };
    },
  });

  const hover = monaco.languages.registerHoverProvider("c", {
    provideHover(model, position) {
      const word = model.getWordAtPosition(position);
      if (!word) return null;

      const doc = HOVER_DOCS[word.word];
      if (!doc) return null;

      return {
        range: {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        },
        contents: [
          { value: `**${doc.title}**` },
          { value: doc.detail },
        ],
      };
    },
  });

  const signature = monaco.languages.registerSignatureHelpProvider("c", {
    signatureHelpTriggerCharacters: ["("],
    signatureHelpRetriggerCharacters: [","],
    provideSignatureHelp(model, position) {
      const line = model.getLineContent(position.lineNumber).slice(0, position.column - 1);

      if (line.endsWith("bpf_map_update_elem(")) {
        return {
          value: {
            signatures: [
              {
                label: "bpf_map_update_elem(map, key, value, flags)",
                parameters: [
                  { label: "map" },
                  { label: "key" },
                  { label: "value" },
                  { label: "flags" },
                ],
              },
            ],
            activeSignature: 0,
            activeParameter: 0,
          },
          dispose: () => undefined,
        };
      }

      if (line.endsWith("bpf_printk(")) {
        return {
          value: {
            signatures: [
              {
                label: "bpf_printk(fmt, ...)",
                parameters: [{ label: "fmt" }, { label: "..." }],
              },
            ],
            activeSignature: 0,
            activeParameter: 0,
          },
          dispose: () => undefined,
        };
      }

      return {
        value: {
          signatures: [],
          activeSignature: 0,
          activeParameter: 0,
        },
        dispose: () => undefined,
      };
    },
  });

  const symbols = monaco.languages.registerDocumentSymbolProvider("c", {
    provideDocumentSymbols(model) {
      const source = model.getValue();
      const items: Monaco.languages.DocumentSymbol[] = [];
      for (const match of source.matchAll(/SEC\("([^"]+)"\)\s*\n?[^\n{]*?\b([A-Za-z_]\w*)\s*\(/g)) {
        const start = model.getPositionAt(match.index ?? 0);
        const end = model.getPositionAt((match.index ?? 0) + match[0].length);
        items.push({
          name: match[2],
          detail: `eBPF hook: ${match[1]}`,
          kind: monaco.languages.SymbolKind.Function,
          range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column),
          selectionRange: new monaco.Range(end.lineNumber, Math.max(1, end.column - match[2].length - 1), end.lineNumber, end.column - 1),
          children: [],
          tags: [],
        });
      }
      for (const match of source.matchAll(/}\s*([A-Za-z_]\w*)\s+SEC\("\.maps"\)/g)) {
        const start = model.getPositionAt(match.index ?? 0);
        const end = model.getPositionAt((match.index ?? 0) + match[0].length);
        items.push({
          name: match[1],
          detail: "eBPF map",
          kind: monaco.languages.SymbolKind.Object,
          range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column),
          selectionRange: new monaco.Range(end.lineNumber, Math.max(1, end.column - match[1].length - 13), end.lineNumber, end.column),
          children: [],
          tags: [],
        });
      }
      return items;
    },
  });

  const definitions = monaco.languages.registerDefinitionProvider("c", {
    provideDefinition(model, position) {
      const word = model.getWordAtPosition(position)?.word;
      if (!word) return null;
      const pattern = new RegExp(`(?:^|\\n)[^;\\n]*\\b${escapeRegex(word)}\\s*\\([^;]*\\)\\s*\\{`, "m");
      const match = pattern.exec(model.getValue());
      if (!match) return null;
      const offset = (match.index ?? 0) + match[0].lastIndexOf(word);
      const start = model.getPositionAt(offset);
      return {
        uri: model.uri,
        range: new monaco.Range(start.lineNumber, start.column, start.lineNumber, start.column + word.length),
      };
    },
  });

  const actions = monaco.languages.registerCodeActionProvider("c", {
    provideCodeActions(model, _range, context) {
      const actions: Monaco.languages.CodeAction[] = [];
      for (const marker of context.markers) {
        const include = marker.message.match(/^Missing #include <([^>]+)>/);
        if (include) {
          actions.push(insertAtTopAction(monaco, model, `#include <${include[1]}>\n`, `Add #include <${include[1]}>`, marker));
        }
        if (marker.message === "Missing GPL license declaration") {
          actions.push({
            title: "Add GPL license declaration",
            kind: "quickfix",
            diagnostics: [marker],
            isPreferred: true,
            edit: {
              edits: [{
                resource: model.uri,
                textEdit: {
                  range: new monaco.Range(model.getLineCount() + 1, 1, model.getLineCount() + 1, 1),
                  text: '\nchar _license[] SEC("license") = "GPL";\n',
                },
                versionId: model.getVersionId(),
              }],
            },
          });
        }
      }
      return { actions, dispose: () => undefined };
    },
  });

  return {
    dispose() {
      completion.dispose();
      hover.dispose();
      signature.dispose();
      symbols.dispose();
      definitions.dispose();
      actions.dispose();
    },
  };
}

function insertAtTopAction(
  monaco: typeof Monaco,
  model: Monaco.editor.ITextModel,
  text: string,
  title: string,
  marker: Monaco.editor.IMarkerData,
): Monaco.languages.CodeAction {
  return {
    title,
    kind: "quickfix",
    diagnostics: [marker],
    isPreferred: true,
    edit: {
      edits: [{
        resource: model.uri,
        textEdit: { range: new monaco.Range(1, 1, 1, 1), text },
        versionId: model.getVersionId(),
      }],
    },
  };
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function toSemanticCompletionKind(
  monaco: typeof Monaco,
  kind: "function" | "type" | "constant" | "field",
) {
  if (kind === "function") return monaco.languages.CompletionItemKind.Function;
  if (kind === "type") return monaco.languages.CompletionItemKind.Struct;
  if (kind === "constant") return monaco.languages.CompletionItemKind.Constant;
  return monaco.languages.CompletionItemKind.Field;
}
