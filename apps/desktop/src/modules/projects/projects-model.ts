/** Pure helpers for the Projects pages: searching, merge targets and labels. */
import type { ProjectInfo, ProjectRow } from "@veyra/contracts";

/** The longest name a project may have (the core refuses longer ones). */
export const MAX_NAME_CHARS = 80;

/** Projects whose name, path or merged paths contain the search text (case-insensitive). */
export function searchProjects(rows: readonly ProjectRow[], text: string): ProjectRow[] {
  const needle = text.trim().toLowerCase();
  if (needle === "") return [...rows];
  return rows.filter((row) =>
    [row.project.label, row.project.path, ...row.project.aliases.map((a) => a.path)].some((s) =>
      s.toLowerCase().includes(needle),
    ),
  );
}

/** The projects another project can be merged into: every other one, by label. */
export function mergeTargets(
  project: ProjectInfo,
  projects: readonly ProjectInfo[],
): ProjectInfo[] {
  return projects.filter((p) => p.id !== project.id).sort((a, b) => a.label.localeCompare(b.label));
}

/** The folder name of a project path, as a short second line under its name. */
export function folderOf(project: ProjectInfo): string | null {
  return project.name === null ? null : project.path;
}
