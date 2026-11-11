import { describe, expect, it } from "vitest";

import { emptyBudget, emptyEntry, fromNano, scheduleOf, toNano } from "./billing-model";

describe("amounts", () => {
  it("converts typed amounts to billionths exactly", () => {
    expect(toNano("20")).toBe(20_000_000_000);
    expect(toNano("19.99")).toBe(19_990_000_000);
    expect(toNano(" 1,250.5 ")).toBe(1_250_500_000_000);
    expect(toNano("0.000000001")).toBe(1);
    // 0.1 + 0.2 style drift cannot happen: digits are joined, not multiplied.
    expect(toNano("0.3")).toBe(300_000_000);
  });

  it("refuses anything that is not a plain non-negative decimal", () => {
    for (const text of ["", "abc", "-5", "1.0000000001", "1e3", "$5"]) {
      expect(toNano(text)).toBeNull();
    }
  });

  it("shows billionths as a plain decimal", () => {
    expect(fromNano(20_000_000_000)).toBe("20");
    expect(fromNano(19_990_000_000)).toBe("19.99");
    expect(fromNano(1)).toBe("0.000000001");
    expect(toNano(fromNano(1_234_567_890_123))).toBe(1_234_567_890_123);
  });
});

describe("drafts", () => {
  it("start as a monthly subscription today, and a budget for all usage", () => {
    expect(emptyEntry("2026-11-12")).toMatchObject({
      kind: "subscription",
      recurrence: "monthly",
      currency: "USD",
      startsOn: "2026-11-12",
      endsOn: null,
    });
    expect(emptyBudget(80)).toMatchObject({ warnPercent: 80, projectId: null });
  });

  it("describe their schedule in words", () => {
    const day = (iso: string) => iso;
    const entry = emptyEntry("2026-11-01");
    expect(scheduleOf(entry, day)).toBe("Monthly from 2026-11-01");
    expect(scheduleOf({ ...entry, endsOn: "2027-10-31" }, day)).toBe(
      "Monthly from 2026-11-01 until 2027-10-31",
    );
    expect(scheduleOf({ ...entry, recurrence: "once" }, day)).toBe("Once on 2026-11-01");
  });
});
