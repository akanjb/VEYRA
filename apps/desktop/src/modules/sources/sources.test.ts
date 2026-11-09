import type { SourceSummary } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { health, overallHealth } from "./health";
import { issueLabel } from "./issues";

const NOW = Date.now();

function source(overrides: Partial<SourceSummary> = {}): SourceSummary {
  return {
    id: 1,
    connectorId: "claude-code",
    displayName: "Claude Code",
    root: "/home/dev/.claude/projects",
    enabled: true,
    documentedFormat: false,
    consentedAt: NOW,
    lastScanAt: NOW,
    lastSuccessAt: NOW,
    lastError: null,
    filesActive: 1,
    filesMissing: 0,
    lastIngestedAt: NOW,
    events: 10,
    recentIssues: [],
    capabilities: [],
    ...overrides,
  };
}

describe("health", () => {
  it("states each condition in words, worst first", () => {
    expect(health(source({ enabled: false, lastError: "boom" }))).toEqual({
      label: "Not enabled",
      tone: "off",
    });
    expect(health(source({ lastError: "boom" })).label).toBe("Last scan failed");
    expect(health(source({ lastSuccessAt: null })).label).toBe("Waiting for first scan");
    expect(health(source()).tone).toBe("ok");
  });

  it("summarises several sources in words, ignoring disabled ones", () => {
    expect(overallHealth([])).toEqual({ label: "No sources enabled", tone: "off" });
    expect(overallHealth([source({ enabled: false })]).tone).toBe("off");
    expect(overallHealth([source(), source({ enabled: false, lastError: "x" })])).toEqual({
      label: "1 source",
      tone: "ok",
    });
    expect(overallHealth([source(), source({ lastError: "x" })])).toEqual({
      label: "2 sources · needs attention",
      tone: "warn",
    });
  });
});

describe("issueLabel", () => {
  it("names known kinds and shows unknown kinds as their identifier", () => {
    expect(issueLabel("malformed-json")).toBe("Lines that are not valid JSON");
    expect(issueLabel("something-new")).toBe("something-new");
  });
});
