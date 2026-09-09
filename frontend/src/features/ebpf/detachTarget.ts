export function buildDetachBody(pinPath: string | null): { pin_path: string | null } {
  if (pinPath !== null && (typeof pinPath !== "string" || !pinPath.trim())) {
    throw new Error("An exact attachment path is required; detach-all must be explicit.");
  }
  return { pin_path: pinPath };
}
