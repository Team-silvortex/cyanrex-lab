export class CyanrexClient {
  constructor(private readonly baseUrl: string) {}

  modules = {
    start: async (name: string) => this.post("/modules/start", { name }),
    stop: async (name: string) => this.post("/modules/stop", { name }),
  };

  events = {
    list: async () => this.get("/events"),
  };

  private async get(path: string) {
    const response = await fetch(`${this.baseUrl}${path}`);
    return response.json();
  }

  private async post(path: string, body: Record<string, unknown>) {
    const response = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    return response.json();
  }
}
