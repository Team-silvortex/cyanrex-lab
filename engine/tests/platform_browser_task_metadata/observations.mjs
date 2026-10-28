// Private fixture observations only. Never expose assertion values or request data to IPC.
export function createObservations() {
  const checks = [];
  return {
    track(check, action) {
      // Handle rejection at registration, not only at a later phase drain. Assertion values
      // can contain credentials or Task text and must never become an unhandled rejection.
      if (!/^(?:request|reply)-[a-z-]+$/.test(check)) throw new Error("invalid private check code");
      checks.push(Promise.resolve().then(action).then(() => null, () => check));
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
