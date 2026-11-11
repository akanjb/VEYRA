import type { ProjectInfo, ProjectRow } from "@veyra/contracts";
import { describe, expect, it } from "vitest";

import { folderOf, mergeTargets, searchProjects } from "./projects-model";

const info = (id: number, path: string, extra: Partial<ProjectInfo> = {}): ProjectInfo => ({
  id,
  path,
  name: null,
  label: path,
  aliases: [],
  mergedInto: null,
  ...extra,
});

const row = (project: ProjectInfo): ProjectRow => ({
  project,
  measures: { tokens: 0, costNanoUsd: 0, requests: 0 },
  sessions: 0,
  activeMs: 0,
  topModel: null,
  lastActiveAt: null,
  dailyTokens: [],
});

const rows = [
  row(
    info(1, "/work/veyra", {
      name: "Veyra",
      label: "Veyra",
      aliases: [{ id: 3, path: "/old/veyra-v1" }],
    }),
  ),
  row(info(2, "/work/shop")),
];

describe("projects model", () => {
  it("searches names, paths and merged folders, ignoring case", () => {
    expect(searchProjects(rows, "VEYRA").map((r) => r.project.id)).toEqual([1]);
    expect(searchProjects(rows, "v1").map((r) => r.project.id)).toEqual([1]);
    expect(searchProjects(rows, "shop").map((r) => r.project.id)).toEqual([2]);
    expect(searchProjects(rows, "  ")).toHaveLength(2);
  });

  it("offers every other project as a merge target, by name", () => {
    const projects = [info(2, "/b"), info(1, "/a"), info(3, "/c")];
    expect(mergeTargets(info(1, "/a"), projects).map((p) => p.id)).toEqual([2, 3]);
  });

  it("shows the folder under a renamed project only", () => {
    expect(folderOf(rows[0]?.project ?? info(0, ""))).toBe("/work/veyra");
    expect(folderOf(info(2, "/work/shop"))).toBeNull();
  });
});
