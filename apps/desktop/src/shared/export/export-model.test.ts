import { describe, expect, it } from "vitest";

import { defaultPreset, namesProjects, pathsLeftOut } from "./export-model";

describe("export choices", () => {
  it("start from the report that matches the page", () => {
    expect(defaultPreset("/projects")).toBe("byProject");
    expect(defaultPreset("/models")).toBe("byModel");
    expect(defaultPreset("/cost")).toBe("costSummary");
    expect(defaultPreset("/usage")).toBe("usageSummary");
    expect(defaultPreset("/")).toBe("daily");
  });

  it("offer project redaction only where projects are listed", () => {
    expect(namesProjects("byProject")).toBe(true);
    expect(namesProjects("daily")).toBe(false);
  });

  it("never pair hashed names with paths", () => {
    expect(pathsLeftOut({ hashProjectNames: true, dropPaths: false })).toBe(true);
    expect(pathsLeftOut({ hashProjectNames: false, dropPaths: true })).toBe(true);
    expect(pathsLeftOut({ hashProjectNames: false, dropPaths: false })).toBe(false);
  });
});
