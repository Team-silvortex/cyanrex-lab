import type * as Monaco from "monaco-editor";

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

export function registerEbpfIntelligence(monaco: typeof Monaco): Monaco.IDisposable {
  const completion = monaco.languages.registerCompletionItemProvider("c", {
    triggerCharacters: ["#", "_", "b", "X"],
    provideCompletionItems(model, position) {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      const suggestions = COMPLETIONS.map((item) => ({
        label: item.label,
        kind: toCompletionKind(monaco, item.kind),
        insertText: item.insertText,
        insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
        detail: item.detail,
        range,
      }));

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

  return {
    dispose() {
      completion.dispose();
      hover.dispose();
      signature.dispose();
    },
  };
}
