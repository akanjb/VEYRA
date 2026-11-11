import { describe, expect, it } from "vitest";

import { TEST_THEME as theme } from "@/shared/charts/test-theme";
import { SAMPLE_OVERVIEW } from "@/shared/testing/sample-overview";

import { projectItems } from "./overview-model";

const overview = SAMPLE_OVERVIEW;

describe("overview model", () => {
  it("labels projects with their paths", () => {
    expect(projectItems(overview, "tokens", theme).map((i) => [i.key, i.label])).toEqual([
      ["1", "/work/a"],
      ["2", "/work/b"],
    ]);
  });
});
