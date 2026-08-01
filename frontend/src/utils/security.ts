const HTML_ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
  "`": "&#96;",
};

export function sanitizeForDisplay(input: string): string {
  return input.replace(/[&<>"'`]/g, (char) => HTML_ESCAPE_MAP[char]);
}

export function parseSafeRedirectPath(
  nextPath: string | undefined,
  fallback: string = "/dashboard",
): string {
  if (!nextPath || nextPath.trim().length === 0) {
    return fallback;
  }

  const decoded = (() => {
    try {
      return decodeURIComponent(nextPath);
    } catch {
      return nextPath;
    }
  })();

  if (!decoded.startsWith("/") || decoded.startsWith("//")) {
    return fallback;
  }

  try {
    const candidate = new URL(decoded, window.location.origin);
    if (candidate.origin !== window.location.origin) {
      return fallback;
    }
    const normalizedPath = `${candidate.pathname}${candidate.search}${candidate.hash}` || "/";
    if (!normalizedPath.startsWith("/") || normalizedPath.startsWith("/login")) {
      return fallback;
    }

    return normalizedPath;
  } catch {
    return fallback;
  }
}
