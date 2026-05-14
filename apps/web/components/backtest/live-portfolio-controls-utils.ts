export function canDirectPublish(pending: string, liveReadinessBlockers: readonly string[]): boolean {
  return pending === "" && liveReadinessBlockers.length === 0;
}

export function canSaveDraft(pending: string): boolean {
  return pending === "";
}
