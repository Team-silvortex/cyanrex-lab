export type EngineEvent = {
  username: string;
  timestamp: string;
  source: string;
  event_type: string;
  category: "kernel" | "platform";
  severity: "success" | "warning" | "error";
  color: "green" | "yellow" | "red";
  payload: Record<string, unknown>;
};

export type EventStreamState = "connecting" | "recovering" | "open" | "closed";
type StreamSocket = Pick<WebSocket, "onopen" | "onclose" | "onerror" | "onmessage" | "close">;
type Runtime = {
  connect: (url: string) => StreamSocket;
  snapshot: (url: string, signal: AbortSignal) => Promise<unknown>;
  later: (callback: () => void, delay: number) => unknown;
  cancel: (timer: unknown) => void;
  now: () => number;
  random: () => number;
};
type Options = {
  socketUrl: string;
  snapshotUrl: string;
  onEvents: (events: EngineEvent[]) => void;
  onState: (state: EventStreamState) => void;
  onGap: () => void;
  accepts?: (event: EngineEvent) => boolean;
  limit?: number;
};

const browserRuntime: Runtime = {
  connect: (url) => new WebSocket(url),
  snapshot: async (url, signal) => {
    const response = await fetch(url, { signal, credentials: "include", cache: "no-store", redirect: "error" });
    if (!response.ok) throw Object.assign(new Error(`HTTP ${response.status}`), { status: response.status });
    return response.json();
  },
  later: (callback, delay) => window.setTimeout(callback, delay),
  cancel: (timer) => window.clearTimeout(timer as number),
  now: () => Date.now(),
  random: () => Math.random(),
};

// The legacy wire format has no cursor or event ID. Reconcile identical records by multiplicity,
// not a Set (which would erase repeated events). This is best-effort, not an exactly-once replay.
function eventKey(event: EngineEvent): string {
  return JSON.stringify(event, (_key, value) => {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return Object.fromEntries(Object.keys(value).sort().map((key) => [key, value[key]]));
    }
    return value;
  });
}

function reconcileSnapshot(snapshot: EngineEvent[], buffered: EngineEvent[], limit: number) {
  const overlap = new Map<string, number>();
  for (const row of snapshot) {
    const key = eventKey(row);
    overlap.set(key, (overlap.get(key) ?? 0) + 1);
  }
  const additions = buffered.filter((row) => {
    const key = eventKey(row), count = overlap.get(key) ?? 0;
    if (count) { overlap.set(key, count - 1); return false; }
    return true;
  });
  // A new live record marks the end of the overlap window. Do not keep a lifetime dedupe set.
  if (additions.length) overlap.clear();
  return { rows: [...snapshot, ...additions].slice(-limit), overlap };
}

export function mergeEventSnapshot(snapshot: EngineEvent[], buffered: EngineEvent[], limit: number) {
  return reconcileSnapshot(snapshot, buffered, limit).rows;
}

function isEvent(value: unknown): value is EngineEvent {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const event = value as EngineEvent;
  return [event.username, event.timestamp, event.source, event.event_type].every((item) => typeof item === "string")
    && ["kernel", "platform"].includes(event.category)
    && ["success", "warning", "error"].includes(event.severity)
    && ["green", "yellow", "red"].includes(event.color)
    && !!event.payload && typeof event.payload === "object" && !Array.isArray(event.payload);
}

// One connection, request and deadline/retry timer at a time. Disposal invalidates every callback;
// filter changes create a new controller so late snapshots cannot overwrite the new selection.
export function startEventStream(options: Options, runtime: Runtime = browserRuntime): () => void {
  const limit = Math.max(1, Math.min(500, options.limit ?? 200));
  let disposed = false, generation = 0, failures = 0;
  let socket: StreamSocket | null = null, abort: AbortController | null = null;
  let timer: unknown = null, liveSince: number | null = null;

  const clearTimer = () => {
    if (timer !== null) runtime.cancel(timer);
    timer = null;
  };
  const cleanConnection = () => {
    generation++;
    clearTimer();
    abort?.abort(); abort = null;
    const previous = socket; socket = null;
    previous?.close();
  };
  const retry = (terminal = false) => {
    if (disposed) return;
    if (liveSince !== null && runtime.now() - liveSince >= 30000) failures = 0;
    liveSince = null;
    cleanConnection();
    options.onGap();
    options.onState(terminal ? "closed" : "recovering");
    if (terminal) return;
    // Jitter is bounded to [base, 1.5 * base], with a hard 30s cap, even after rapid reopen/close.
    const base = Math.min(20000, 500 * 2 ** Math.min(failures++, 6));
    timer = runtime.later(connect, Math.min(30000, base * (1 + runtime.random() / 2)));
  };
  const connect = () => {
    if (disposed) return;
    clearTimer();
    const current = ++generation;
    const active = () => !disposed && current === generation;
    let loading = true, buffered: EngineEvent[] = [], rows: EngineEvent[] = [];
    let overlap = new Map<string, number>();
    options.onState(failures ? "recovering" : "connecting");
    try { socket = runtime.connect(options.socketUrl); } catch { retry(); return; }
    timer = runtime.later(() => { if (active()) retry(); }, 10000);
    socket.onopen = () => {
      if (!active()) return;
      clearTimer();
      abort = new AbortController();
      timer = runtime.later(() => { if (active()) retry(); }, 10000);
      void runtime.snapshot(options.snapshotUrl, abort.signal).then((snapshot) => {
        if (!active()) return;
        if (!Array.isArray(snapshot) || !snapshot.every(isEvent)) throw new Error("invalid event snapshot");
        clearTimer(); abort = null;
        const selected = snapshot.filter((row) => !options.accepts || options.accepts(row)).slice(-limit);
        ({ rows, overlap } = reconcileSnapshot(selected, buffered, limit));
        buffered = []; loading = false; liveSince = runtime.now();
        options.onEvents(rows);
        options.onState("open");
      }).catch((error: unknown) => {
        if (!active()) return;
        const status = (error as { status?: number } | null)?.status;
        retry(status === 401 || status === 403);
      });
    };
    socket.onmessage = (message) => {
      if (!active()) return;
      if (typeof message.data !== "string") { options.onGap(); return; }
      let event: unknown;
      try { event = JSON.parse(message.data); } catch { options.onGap(); return; }
      if (!isEvent(event)) { options.onGap(); return; }
      if (options.accepts && !options.accepts(event)) return;
      if (loading) {
        if (buffered.length >= limit) { retry(); return; }
        buffered.push(event);
        return;
      }
      if (overlap.size) {
        const key = eventKey(event), count = overlap.get(key) ?? 0;
        if (count) { overlap.set(key, count - 1); return; }
        overlap.clear();
      }
      rows = [...rows, event].slice(-limit);
      options.onEvents(rows);
    };
    socket.onclose = (event) => { if (active()) retry(event.code === 1008); };
    socket.onerror = () => { if (active()) retry(); };
  };
  connect();
  return () => { disposed = true; cleanConnection(); };
}
