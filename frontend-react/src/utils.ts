/** Parses a route param as a positive integer; `null` when it is not one. */
export function parsePositiveInt(value: string | null | undefined): number | null {
  if (value === null || value === undefined || !/^[1-9]\d*$/.test(value)) return null
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) ? parsed : null
}

/** Formats an ISO timestamp as `YYYY-MM-DD` in the viewer's local time. */
export function formatDate(iso: string): string {
  const date = new Date(iso)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

/** Only same-site paths are valid post-login targets (prevents open redirects). */
export function safeNextPath(next: string | null): string | null {
  return next !== null && next.startsWith('/') && !next.startsWith('//') ? next : null
}
