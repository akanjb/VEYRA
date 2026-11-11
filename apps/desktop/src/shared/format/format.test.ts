import { describe, expect, it } from "vitest";

import {
  MISSING,
  ago,
  basename,
  change,
  compact,
  dateTime,
  day,
  duration,
  integer,
  percent,
  shortId,
  times,
  usd,
} from "./format";

const L = "en-US";

describe("format", () => {
  it("never renders unknown values as zero", () => {
    for (const formatted of [
      compact(null, L),
      integer(undefined, L),
      usd(null),
      percent(Number.NaN, L),
      change(null, L),
      duration(undefined),
      times(null, L),
    ]) {
      expect(formatted).toBe(MISSING);
    }
  });

  it("formats counts compactly and in full", () => {
    expect(compact(950, L)).toBe("950");
    expect(compact(12_400, L)).toBe("12.4K");
    expect(compact(3_120_000, L)).toBe("3.1M");
    expect(integer(12_400_315, L)).toBe("12,400,315");
    expect(times(2.44, L)).toBe("2.4×");
  });

  it("formats nano-USD, marking estimates and sub-cent amounts", () => {
    expect(usd(4_412_300_000_000, { locale: L })).toBe("$4,412.30");
    expect(usd(4_412_300_000_000, { estimated: true, locale: L })).toBe("≈$4,412.30");
    expect(usd(5_000_000, { locale: L })).toBe("<$0.01");
    expect(usd(0, { locale: L })).toBe("$0.00");
    expect(usd(25_400_000_000_000, { compact: true, locale: L })).toBe("$25.4K");
  });

  it("formats ratios and signed changes", () => {
    expect(percent(0.0962, L)).toBe("9.6%");
    expect(percent(0.4231, L)).toBe("42%");
    expect(change(0.38, L)).toBe("+38%");
    expect(change(-0.042, L)).toBe("−4.2%");
    expect(change(0, L)).toBe("0%");
  });

  it("formats durations and relative times", () => {
    expect(duration(30_000)).toBe("30s");
    expect(duration(45 * 60_000)).toBe("45m");
    expect(duration((3 * 60 + 12) * 60_000)).toBe("3h 12m");
    expect(duration(52 * 3_600_000)).toBe("2d 4h");
    const now = 1_000_000_000;
    expect(ago(now - 2_000, now)).toBe("just now");
    expect(ago(now - 12_000, now)).toBe("12s ago");
    expect(ago(now - 5 * 60_000, now)).toBe("5m ago");
    expect(ago(now - 3 * 86_400_000, now)).toBe("3d ago");
  });

  it("formats calendar days without shifting them across time zones", () => {
    expect(day("2026-11-08", L)).toBe("Nov 8, 2026");
    expect(dateTime(Date.UTC(2026, 10, 8, 14, 32), "UTC", L)).toBe("Nov 8, 2026, 02:32 PM");
  });
});

describe("identifiers", () => {
  it("shortens paths and session keys", () => {
    expect(basename("c:/users/dev/code/veyra")).toBe("veyra");
    expect(basename("C:\\work\\demo\\")).toBe("demo");
    expect(shortId("claude-code:1f2e3d4c-aaaa-bbbb")).toBe("1f2e3d4c");
    expect(shortId("claude-code:1f2e3d4c-aaaa:a1")).toBe("1f2e3d4c · a1");
  });
});
