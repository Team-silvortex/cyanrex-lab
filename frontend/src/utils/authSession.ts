export async function logoutSession(engineUrl: string, signal?: AbortSignal): Promise<void> {
  const deadline = AbortSignal.timeout(10000);
  const response = await fetch(`${engineUrl}/auth/logout`, {
    method: "POST",
    credentials: "include",
    cache: "no-store",
    redirect: "error",
    signal: signal ? AbortSignal.any([signal, deadline]) : deadline,
  });
  if (!response.ok) throw new Error("Logout was not confirmed");
  const result = await response.json() as { ok?: unknown } | null;
  if (result?.ok !== true) throw new Error("Logout was not confirmed");
}
