const DEFAULT_ENGINE_ORIGIN = "http://localhost:8080";

function resolveEngineOrigin(configuredUrl) {
  const candidate = configuredUrl?.trim() || DEFAULT_ENGINE_ORIGIN;
  try {
    const parsed = new URL(candidate);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return DEFAULT_ENGINE_ORIGIN;
    }
    return parsed.origin;
  } catch {
    return DEFAULT_ENGINE_ORIGIN;
  }
}

function buildContentSecurityPolicy(configuredUrl) {
  const engineOrigin = resolveEngineOrigin(configuredUrl);
  return [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval' blob:",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: blob:",
    "font-src 'self' data:",
    `connect-src 'self' ${engineOrigin} ws: wss:`,
    "worker-src 'self' blob:",
    "child-src 'self' blob:",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");
}

module.exports = {
  DEFAULT_ENGINE_ORIGIN,
  buildContentSecurityPolicy,
  resolveEngineOrigin,
};
