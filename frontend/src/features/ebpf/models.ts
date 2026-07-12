export type EbpfRunResponse = {
  success: boolean;
  stage: string;
  message: string;
  compile_stdout: string;
  compile_stderr: string;
  load_stdout: string;
  load_stderr: string;
  pin_path?: string | null;
};

export type EbpfRuntimeBackend = "bpftool" | "aya";

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
