const DEFAULT_ENGINE_URL = "http://localhost:8080";

export function getEngineUrl(): string {
  const configured = process.env.NEXT_PUBLIC_ENGINE_URL?.trim();
  return (configured || DEFAULT_ENGINE_URL).replace(/\/+$/, "");
}

export function toWebSocketUrl(baseHttpUrl: string, path: string): string {
  const url = new URL(path, `${baseHttpUrl.replace(/\/+$/, "")}/`);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}
