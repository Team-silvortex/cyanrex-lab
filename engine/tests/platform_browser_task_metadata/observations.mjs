// Private fixture observations only. Never expose assertion values or request data to IPC.
const responseParts = new Set(["event-timeout", "event-failed", "body-resource-missing",
  "body-protocol", "body-failed", "json-invalid"]);
function responseFailure(part) {
  const error = new Error("private browser response observation failed");
  error.observationPart = part;
  return error;
}

// Read exactly the observed reply once. Keep transport retrieval separate from JSON parsing so
// CI can identify its failing boundary without logging private messages, bytes or assertion values.
export async function readObservedJson(incoming) {
  let response, raw;
  try { response = await incoming; }
  catch (error) { throw responseFailure(error?.name === "TimeoutError" ? "event-timeout" : "event-failed"); }
  try { raw = await response.text(); }
  catch (error) {
    const message = typeof error?.message === "string" ? error.message : "";
    const missing = /No (?:resource|data) (?:with given identifier|found for resource)/.test(message);
    throw responseFailure(missing ? "body-resource-missing"
      : message.includes("Network.getResponseBody") ? "body-protocol" : "body-failed");
  }
  let body;
  try { body = JSON.parse(raw); }
  catch { throw responseFailure("json-invalid"); }
  return { status: response.status(), body };
}

export function createObservations() {
  const checks = [];
  return {
    track(check, action) {
      // Handle rejection at registration, not only at a later phase drain. Assertion values
      // can contain credentials or Task text and must never become an unhandled rejection.
      if (!/^(?:request|reply)-[a-z-]+$/.test(check)) throw new Error("invalid private check code");
      checks.push(Promise.resolve().then(action).then(() => null, error =>
        responseParts.has(error?.observationPart) ? `${check}-${error.observationPart}` : check));
    },
    async drain() {
      let offset = 0, failed = null;
      // A header check can enqueue another observation while awaited. Do not treat an
      // earlier Promise.all snapshot as a complete network-observation boundary.
      while (offset < checks.length) {
        const batch = checks.slice(offset); offset = checks.length;
        const results = await Promise.all(batch);
        failed ??= results.find(check => check !== null) ?? null;
      }
      if (failed) {
        const error = new Error("private browser observation failed");
        error.check = failed;
        throw error;
      }
    },
  };
}
