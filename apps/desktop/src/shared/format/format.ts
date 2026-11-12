/**
 * Number, money, ratio, duration and date formatting: the single source for the
 * dashboard (docs/planning/03-ux.md §5.1). Unknown values render as an em dash, never 0.
 */

/** Rendered for values the source did not report. */
export const MISSING = "—";

const NANO_PER_USD = 1_000_000_000;

type Num = number | null | undefined;

const isMissing = (value: Num): value is null | undefined =>
  value === null || value === undefined || Number.isNaN(value);

/** Compact large numbers: 950, 12.4K, 3.1M, 2.05B. */
export function compact(value: Num, locale?: string): string {
  if (isMissing(value)) return MISSING;
  return new Intl.NumberFormat(locale, {
    notation: "compact",
    maximumFractionDigits: Math.abs(value) >= 1_000 ? 1 : 0,
  }).format(value);
}

const BYTE_UNITS = ["KB", "MB", "GB", "TB"];

/** A size in bytes, in binary units: 512 B, 4.2 KB, 1.3 GB. */
export function bytes(value: Num, locale?: string): string {
  if (isMissing(value)) return MISSING;
  if (Math.abs(value) < 1024) return `${integer(value, locale)} B`;
  let scaled = value;
  let unit = 0;
  do {
    scaled /= 1024;
    unit += 1;
  } while (Math.abs(scaled) >= 1024 && unit < BYTE_UNITS.length);
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(scaled);
  return `${number} ${BYTE_UNITS[unit - 1] ?? "TB"}`;
}

/** Full precision with grouping: 12,400,315. */
export function integer(value: Num, locale?: string): string {
  if (isMissing(value)) return MISSING;
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value);
}

/**
 * US dollars from nano-USD. API-equivalent values are estimates and get the "≈" prefix.
 * Sub-cent amounts show as "<$0.01" rather than rounding to zero.
 */
export function usd(
  nanoUsd: Num,
  options: { estimated?: boolean; compact?: boolean; locale?: string } = {},
): string {
  if (isMissing(nanoUsd)) return MISSING;
  const dollars = nanoUsd / NANO_PER_USD;
  const prefix = options.estimated === true ? "≈" : "";
  if (dollars !== 0 && Math.abs(dollars) < 0.01) return `${prefix}<$0.01`;
  const formatter = new Intl.NumberFormat(options.locale, {
    style: "currency",
    currency: "USD",
    ...(options.compact === true && Math.abs(dollars) >= 10_000
      ? { notation: "compact", maximumFractionDigits: 1 }
      : { minimumFractionDigits: 2, maximumFractionDigits: 2 }),
  });
  return prefix + formatter.format(dollars);
}

/** A published price from micro-USD per million tokens: "$2.50 / MTok". */
export function pricePerMTok(microUsd: Num, locale?: string): string {
  if (isMissing(microUsd)) return MISSING;
  const formatter = new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  });
  return `${formatter.format(microUsd / 1_000_000)} / MTok`;
}

/** A price multiplier from basis points (10 000 is 1×): "0.5×", "1.25×". */
export function multiplier(basisPoints: Num, locale?: string): string {
  if (isMissing(basisPoints)) return MISSING;
  return (
    new Intl.NumberFormat(locale, { maximumFractionDigits: 4 }).format(basisPoints / 10_000) + "×"
  );
}

/** A 0–1 ratio as a percentage, with one decimal below 10%. */
export function percent(ratio: Num, locale?: string): string {
  if (isMissing(ratio)) return MISSING;
  const pct = ratio * 100;
  return (
    new Intl.NumberFormat(locale, {
      maximumFractionDigits: Math.abs(pct) < 10 ? 1 : 0,
    }).format(pct) + "%"
  );
}

/** A multiple, such as a reuse ratio: "2.4×". */
export function times(value: Num, locale?: string): string {
  if (isMissing(value)) return MISSING;
  return (
    new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(
      value,
    ) + "×"
  );
}

/** A relative change with an explicit sign: "+38%", "−4.2%". */
export function change(relative: Num, locale?: string): string {
  if (isMissing(relative)) return MISSING;
  if (relative === 0) return "0%";
  const sign = relative > 0 ? "+" : "−";
  return sign + percent(Math.abs(relative), locale);
}

/** A duration: "3h 12m", "45m", "30s", "2d 4h". */
export function duration(ms: Num): string {
  if (isMissing(ms)) return MISSING;
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${String(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${String(hours)}h ${String(minutes % 60)}m`;
  return `${String(Math.floor(hours / 24))}d ${String(hours % 24)}h`;
}

/** A local calendar day (`YYYY-MM-DD`) as "8 Nov 2026". */
export function day(isoDate: string, locale?: string): string {
  const [y, m, d] = isoDate.split("-").map(Number);
  if (y === undefined || m === undefined || d === undefined) return isoDate;
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  }).format(Date.UTC(y, m - 1, d));
}

/** A calendar month (`YYYY-MM`) as "November 2026". */
export function month(isoMonth: string, locale?: string): string {
  const [y, m] = isoMonth.split("-").map(Number);
  if (y === undefined || m === undefined || Number.isNaN(m)) return isoMonth;
  return new Intl.DateTimeFormat(locale, {
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(Date.UTC(y, m - 1, 1));
}

/** An instant as local date and time: "8 Nov 2026, 14:32". */
export function dateTime(epochMs: number, timeZone?: string, locale?: string): string {
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    ...(timeZone === undefined ? {} : { timeZone }),
  }).format(epochMs);
}

/** An instant as local time of day: "14:32:05". */
export function time(epochMs: number, timeZone?: string, locale?: string): string {
  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    ...(timeZone === undefined ? {} : { timeZone }),
  }).format(epochMs);
}

/** How long ago: "just now", "12s ago", "5m ago", "3h ago", "2d ago". */
export function ago(epochMs: number, now: number = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - epochMs) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${String(seconds)}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `${String(hours)}h ago`;
  return `${String(Math.floor(hours / 24))}d ago`;
}

/** The last segment of a path ("c:/code/veyra" → "veyra"). */
export function basename(path: string): string {
  return (
    path
      .split(/[\\/]/)
      .filter((s) => s !== "")
      .at(-1) ?? path
  );
}

/** A short form of a source-scoped id ("claude-code:1f2e…:a1" → "1f2e3d4c · a1"). */
export function shortId(key: string): string {
  const [, id = key, ...rest] = key.split(":");
  return [id.slice(0, 8), ...rest].join(" · ");
}
