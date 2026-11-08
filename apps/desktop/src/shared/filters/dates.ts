/** Civil-date arithmetic on `YYYY-MM-DD` strings (time-zone free). */

const ISO = /^(\d{4})-(\d{2})-(\d{2})$/;

/** Whether `value` is a real calendar date in `YYYY-MM-DD` form. */
export function isIsoDate(value: string): boolean {
  const m = ISO.exec(value);
  if (m === null) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const date = new Date(Date.UTC(y, mo - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === mo - 1 && date.getUTCDate() === d;
}

function toUtc(value: string): number {
  const [y = 1970, m = 1, d = 1] = value.split("-").map(Number);
  return Date.UTC(y, m - 1, d);
}

function fromUtc(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

/** `value` shifted by `days` (negative goes back). */
export function addDays(value: string, days: number): string {
  return fromUtc(toUtc(value) + days * 86_400_000);
}

/** Days from `from` to `to`, inclusive of both. */
export function daysInclusive(from: string, to: string): number {
  return Math.round((toUtc(to) - toUtc(from)) / 86_400_000) + 1;
}

/** The first day of `value`'s month. */
export function monthStart(value: string): string {
  return `${value.slice(0, 7)}-01`;
}

/** The first day of `value`'s year. */
export function yearStart(value: string): string {
  return `${value.slice(0, 4)}-01-01`;
}

/** Today's date in a time zone. */
export function todayIn(timeZone: string, now: Date = new Date()): string {
  // en-CA formats dates as YYYY-MM-DD.
  return new Intl.DateTimeFormat("en-CA", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(now);
}

/** The system time zone (IANA name). */
export function systemTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}
