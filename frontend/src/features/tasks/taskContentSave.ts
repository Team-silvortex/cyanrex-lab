/** Pure one-shot save planner. Supplied response agreement is not commit provenance. */
import { parseTaskDraft, serializeTaskDraft, type TaskDraft } from "./taskDraft";
import * as contract from "./taskSaveContract";
import type { Allocation, ArtifactReference, Baseline, DraftGuard, Manifest } from "./taskSaveContract";

export type SaveStopReason = "cancelled" | "conflict" | "unconfirmed" | "rejected" | "stale";
export type ContentSaveStep = Readonly<{
  kind: "publish"; index: number;
  body: Readonly<{ schema_version: 1; artifact_id: string; revision_id: string; title: string; text: string }>;
}> | Readonly<{
  kind: "replace"; task_id: string;
  body: Readonly<{ schema_version: 1; expected_revision: number; manifest: Manifest }>;
}>;
type PublicationStep = Extract<ContentSaveStep, { kind: "publish" }>;
type ReplacementStep = Extract<ContentSaveStep, { kind: "replace" }>;
type TaskBoundPublicationStep = Readonly<{
  kind: "publish"; index: number; task_id: string;
  body: Readonly<{ schema_version: 1; expected: Baseline; artifact_id: string; revision_id: string; text: string }>;
}>;
export type TaskBoundContentSaveStep = TaskBoundPublicationStep | ReplacementStep;
type Pending = Readonly<{ kind: "publish"; index: number; artifact_id: string; revision_id: string }>
  | Readonly<{ kind: "replace"; task_id: string; expected_revision: number }>;
export type ContentSaveProgress = Readonly<{
  state: "ready" | "pending" | "complete" | "stopped"; confirmedArtifacts: number;
  taskConfirmed: boolean; pending: Pending | null; reason: SaveStopReason | "invalid_confirmation" | null;
}>;
export interface TaskContentSaveAttempt<Step = ContentSaveStep> {
  next(currentGuard: DraftGuard): Step | null;
  confirm(step: Step, response: unknown, currentGuard: DraftGuard): boolean;
  stop(reason: SaveStopReason): void;
  progress(): ContentSaveProgress;
}
export type TaskBoundContentSaveAttempt = TaskContentSaveAttempt<TaskBoundContentSaveStep>;
type Publish = PublicationStep | TaskBoundPublicationStep;
type PlannedItem<P extends Publish> = Readonly<{ step: P; digest: string; byteLength: number }>;
type PublicationFormat<P extends Publish> = Readonly<{
  step(draft: TaskDraft, baseline: Baseline, index: number, allocation: Allocation): P;
  title(draft: TaskDraft, baseline: Baseline): string;
}>;
const encoder = new TextEncoder();
const reasons = new Set<SaveStopReason>(["cancelled", "conflict", "unconfirmed", "rejected", "stale"]);

class Attempt<P extends Publish> implements TaskContentSaveAttempt<P | ReplacementStep> {
  #draft: TaskDraft;
  #baseline: Baseline;
  #guard: DraftGuard;
  #items: readonly PlannedItem<P>[];
  #publicationTitle: string;
  #references: ArtifactReference[] = [];
  #step: P | ReplacementStep | null = null;
  #state: ContentSaveProgress["state"] = "ready";
  #reason: ContentSaveProgress["reason"] = null;

  constructor(draft: TaskDraft, baseline: Baseline, guard: DraftGuard, items: readonly PlannedItem<P>[], title: string) {
    this.#draft = draft; this.#baseline = baseline; this.#guard = guard; this.#items = items;
    this.#publicationTitle = title;
  }
  #current(value: DraftGuard): boolean {
    try { if (contract.same(contract.guard(value), this.#guard)) return true; } catch { /* stop closed */ }
    this.#state = "stopped"; this.#reason = "stale";
    return false;
  }
  next(currentGuard: DraftGuard): P | ReplacementStep | null {
    if (this.#state === "stopped" || this.#state === "complete") return null;
    if (!this.#current(currentGuard)) return null;
    if (this.#state === "pending") throw new Error("taskSave.busy");
    const index = this.#references.length;
    if (index < this.#items.length) {
      this.#step = this.#items[index].step;
    } else {
      const manifest = contract.manifest({ schema_version: 1, title: this.#draft.title,
        payload: this.#draft.payload.map((item, position) => ({ kind: "text", filename: item.filename,
          language: item.language, artifact: this.#references[position] })) });
      const body = Object.freeze({ schema_version: 1 as const,
        expected_revision: this.#baseline.task.revision, manifest });
      if (encoder.encode(JSON.stringify(body)).length > 64 * 1024) return contract.invalid();
      this.#step = Object.freeze({ kind: "replace", task_id: this.#baseline.task.reference.task_id, body });
    }
    // Handoff is one-shot, not evidence that any transport dispatched this descriptor.
    this.#state = "pending";
    return this.#step;
  }
  confirm(step: P | ReplacementStep, response: unknown, currentGuard: DraftGuard): boolean {
    if (this.#state !== "pending" || step !== this.#step) return false;
    if (!this.#current(currentGuard)) return false;
    try {
      if (step.kind === "publish") {
        const planned = this.#items[step.index];
        const pin = contract.publicationResponse(response, {
          reference: Object.freeze({ workspace: this.#baseline.task.reference.workspace,
            artifact_id: step.body.artifact_id, revision_id: step.body.revision_id, sha256: planned.digest }),
          owner: this.#baseline.task.owner, title: this.#publicationTitle, byte_length: planned.byteLength,
        });
        this.#references.push(pin);
        this.#state = "ready";
      } else {
        contract.replacementResponse(response, this.#baseline.task, step.body.manifest);
        this.#state = "complete";
      }
      this.#step = null;
      return true;
    } catch {
      this.#state = "stopped"; this.#reason = "invalid_confirmation";
      return false;
    }
  }
  stop(reason: SaveStopReason): void {
    if (!reasons.has(reason)) return contract.invalid();
    if (this.#state === "complete" || this.#state === "stopped") return;
    this.#state = "stopped"; this.#reason = reason;
  }
  progress(): ContentSaveProgress {
    const step = this.#step;
    const pending: Pending | null = step === null ? null : step.kind === "publish"
      ? Object.freeze({ kind: "publish", index: step.index,
        artifact_id: step.body.artifact_id, revision_id: step.body.revision_id })
      : Object.freeze({ kind: "replace", task_id: step.task_id, expected_revision: step.body.expected_revision });
    return Object.freeze({ state: this.#state, confirmedArtifacts: this.#references.length,
      taskConfirmed: this.#state === "complete", pending, reason: this.#reason });
  }
}

/** Existing manual Drafts only. New Tasks first require a separately confirmed empty-create baseline.
 * No request, token, storage, retry or recovery capability is supplied by this factory.
 */
export async function prepareTaskContentSave(
  draft: TaskDraft, baseline: Baseline, guard: DraftGuard, allocations: readonly Allocation[],
): Promise<TaskContentSaveAttempt> {
  return prepare(draft, baseline, guard, allocations, {
    step: (source, _original, index, allocation) => Object.freeze({ kind: "publish", index,
      body: Object.freeze({ schema_version: 1, ...allocation, title: source.title, text: source.payload[index].text }) }),
    title: source => source.title,
  });
}

/** Explicit Task-bound descriptors. Each publication pins the unchanged original full baseline;
 * its Artifact title is the original Task title. Only the later replacement uses the edited title.
 * This pure factory neither authenticates nor dispatches either operation.
 */
export async function prepareTaskBoundContentSave(
  draft: TaskDraft, baseline: Baseline, guard: DraftGuard, allocations: readonly Allocation[],
): Promise<TaskBoundContentSaveAttempt> {
  return prepare(draft, baseline, guard, allocations, {
    step: (source, original, index, allocation) => Object.freeze({ kind: "publish", index,
      task_id: original.task.reference.task_id, body: Object.freeze({ schema_version: 1,
        expected: original, ...allocation, text: source.payload[index].text }) }),
    title: (_source, original) => original.task.title,
  });
}

async function prepare<P extends Publish>(
  draft: TaskDraft, baseline: Baseline, guard: DraftGuard, allocations: readonly Allocation[], format: PublicationFormat<P>,
): Promise<TaskContentSaveAttempt<P | ReplacementStep>> {
  // Own all data before the first asynchronous digest; caller edits never alter this attempt.
  const frozenDraft = parseTaskDraft(encoder.encode(serializeTaskDraft(draft)));
  contract.title(frozenDraft.title);
  const frozenBaseline = contract.baseline(baseline), frozenGuard = contract.guard(guard);
  if (frozenBaseline.task.revision === Number.MAX_SAFE_INTEGER) return contract.invalid();
  const targets = contract.list(allocations).map(contract.allocation);
  if (targets.length !== frozenDraft.payload.length
    || new Set(targets.map(target => target.artifact_id)).size !== targets.length
    || new Set(targets.map(target => target.revision_id)).size !== targets.length) return contract.invalid();
  // All request envelopes are bounded before even local hashing begins.
  const steps = targets.map((allocation, index) => format.step(frozenDraft, frozenBaseline, index, allocation));
  for (const step of steps) {
    if (encoder.encode(JSON.stringify(step.body)).length > 2 * 1024 * 1024) return contract.invalid();
  }
  const items: PlannedItem<P>[] = [];
  for (const [index, item] of frozenDraft.payload.entries()) {
    const bytes = encoder.encode(item.text);
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    items.push(Object.freeze({ step: steps[index], byteLength: bytes.length,
      digest: Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("") }));
  }
  return new Attempt(frozenDraft, frozenBaseline, frozenGuard, Object.freeze(items), format.title(frozenDraft, frozenBaseline));
}
