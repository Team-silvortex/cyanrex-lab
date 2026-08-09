export type EbpfRunResponse = {
  success: boolean;
  stage: string;
  message: string;
  compile_stdout: string;
  compile_stderr: string;
  load_stdout: string;
  load_stderr: string;
  pin_path?: string | null;
  debug?: EbpfDebugInfo | null;
};

export type EbpfDebugRejectedBreakpoint = {
  line: number;
  reason: string;
};

export type EbpfDebugInfo = {
  mode: "kernel-trace" | string;
  session_id?: string | null;
  requested_lines: number[];
  instrumented_lines: number[];
  rejected: EbpfDebugRejectedBreakpoint[];
};

export type EbpfBreakpointHit = {
  line: number;
  timestamp: string;
};

export type EbpfRuntimeBackend = "bpftool" | "aya";

export type EbpfCompilerDiagnostic = {
  line: number;
  column: number;
  end_column: number;
  severity: "error" | "warning" | "note";
  message: string;
};

export type EbpfCheckResponse = {
  ok: boolean;
  message: string;
  diagnostics: EbpfCompilerDiagnostic[];
  stdout: string;
  stderr: string;
};

export type EbpfCheckBackend = {
  agent_id: string;
  isolation: "container" | "virtual_machine" | "dedicated_host" | "shared_kernel";
  state: "healthy" | "degraded" | "draining" | "offline";
  available_slots: number;
  max_concurrent: number;
};

export type EbpfCheckBackendInventory = {
  local_available: boolean;
  agents: EbpfCheckBackend[];
};

export type EbpfRemoteCheckResponse = {
  job_id: string;
  state: "queued" | "claimed" | "cancel_requested" | "succeeded" | "failed" | "cancelled" | "expired";
  agent_id?: string | null;
  message: string;
  result?: EbpfCheckResponse | null;
};

export type EbpfCompilerTarget = "local" | `agent:${string}`;

export type EbpfCompletionItem = {
  label: string;
  insert_text: string;
  detail: string;
  kind: "function" | "type" | "constant" | "field";
};

export type EbpfCompletionResponse = {
  ok: boolean;
  items: EbpfCompletionItem[];
  message: string;
};

export type EbpfDetachResponse = {
  ok: boolean;
  message: string;
  detached: string[];
  clean?: boolean;
  safety_notes?: string[];
};

export type EbpfAttachmentDetail = {
  pin_path: string;
  source: string;
  program_name: string;
};

export type EbpfAttachmentDetailListResponse = {
  attachments: EbpfAttachmentDetail[];
};

export type EbpfTemplate = {
  id: string;
  name: string;
  description: string;
  capability: string;
  category?: string;
  code: string;
};

export type UserScript = {
  id: string;
  username: string;
  title: string;
  script: string;
  created_at: string;
  updated_at: string;
};

export type SelectedHeaderMetadata = {
  id: string;
  include_hint: string;
  local_path: string;
  downloaded: boolean;
};

export type HeaderSelectionMetadata = {
  selected_headers: SelectedHeaderMetadata[];
};

export const SAMPLE_EBPF = `#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";`;

export const MAX_UPLOAD_BYTES = 256 * 1024;
