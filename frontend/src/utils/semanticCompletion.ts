import type * as Monaco from "monaco-editor";
import type { EbpfCompletionItem, EbpfCompletionResponse } from "../features/ebpf/models";

const CACHE_TTL_MS = 5_000;
const CACHE_LIMIT = 18;

// One owner/editor and header context per registration. No global request sharing.
export function createSemanticCompletion(
  editor: Monaco.editor.IStandaloneCodeEditor, engineUrl: string, headerContextKey: string,
) {
  let disposed = false;
  let generation = 0;
  let active: AbortController | null = null;
  const cache = new Map<string, { expiresAt: number; items: EbpfCompletionItem[] }>();
  const owns = (model: Monaco.editor.ITextModel) => !disposed && !model.isDisposed() && editor.getModel() === model;
  const cancel = () => { generation++; active?.abort(); active = null; };
  const changed = editor.onDidChangeModel(() => { cancel(); cache.clear(); });
  const removed = editor.onDidDispose(dispose);

  function dispose() {
    if (disposed) return;
    disposed = true;
    cancel(); cache.clear(); changed.dispose(); removed.dispose();
  }

  async function request(
    model: Monaco.editor.ITextModel, position: Monaco.Position, token: Monaco.CancellationToken,
  ): Promise<EbpfCompletionItem[] | null> {
    if (!owns(model) || token.isCancellationRequested) return null;
    cancel();
    const current = generation, version = model.getVersionId();
    const isCurrent = () => owns(model) && !token.isCancellationRequested
      && generation === current && model.getVersionId() === version;
    const code = model.getValue();
    if (!code.trim() || new TextEncoder().encode(code).byteLength > 256 * 1024) return [];
    const key = JSON.stringify([engineUrl, headerContextKey, model.uri.toString(), code, position.lineNumber, position.column]);
    const cached = cache.get(key);
    if (cached && cached.expiresAt > Date.now()) return cached.items;
    const controller = new AbortController(); active = controller;
    const cancellation = token.onCancellationRequested(() => controller.abort());
    const content = model.onDidChangeContent(() => controller.abort());
    const removedModel = model.onWillDispose(cancel);
    const timer = setTimeout(() => controller.abort(new DOMException("Semantic completion timed out", "TimeoutError")), 10_000);
    try {
      const response = await fetch(`${engineUrl}/ebpf/complete`, {
        method: "POST", headers: { "Content-Type": "application/json" },
        credentials: "include", cache: "no-store", redirect: "error", signal: controller.signal,
        body: JSON.stringify({ code, line: position.lineNumber, column: position.column }),
      });
      if (!response.ok) return isCurrent() ? [] : null;
      const result = await response.json() as EbpfCompletionResponse;
      if (!isCurrent()) return null;
      controller.signal.throwIfAborted();
      if (result?.ok !== true || !Array.isArray(result.items) || !result.items.every(item => item
        && typeof item.label === "string" && typeof item.insert_text === "string" && typeof item.detail === "string"
        && ["function", "type", "constant", "field"].includes(item.kind))) return [];
      if (result.items.length) {
        for (const [cacheKey, value] of cache) if (value.expiresAt <= Date.now()) cache.delete(cacheKey);
        cache.delete(key);
        cache.set(key, { expiresAt: Date.now() + CACHE_TTL_MS, items: result.items });
        if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!);
      }
      return result.items;
    } catch {
      // Offline/deadline: keep static snippets. Obsolete context: publish nothing.
      return isCurrent() ? [] : null;
    } finally {
      clearTimeout(timer); cancellation.dispose(); content.dispose(); removedModel.dispose();
      if (active === controller) active = null;
    }
  }
  return { owns, request, cancel, dispose };
}
