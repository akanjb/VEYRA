/** Report choices: presets, formats and redaction, as the core defines them (ADR-032). */
import type { ExportFormat, ExportPreset, ExportRedaction } from "@veyra/contracts";

export const PRESETS: readonly { value: ExportPreset; label: string }[] = [
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
  { value: "byProject", label: "Per project" },
  { value: "byModel", label: "Per model" },
  { value: "usageSummary", label: "Usage by application" },
  { value: "costSummary", label: "Cost summary" },
];

export const FORMATS: readonly { value: ExportFormat; label: string }[] = [
  { value: "csv", label: "CSV" },
  { value: "csvExcel", label: "CSV for Excel" },
  { value: "json", label: "JSON" },
];

/** The report a page's export starts with. */
export function defaultPreset(path: string): ExportPreset {
  switch (path) {
    case "/projects":
      return "byProject";
    case "/models":
      return "byModel";
    case "/cost":
      return "costSummary";
    case "/usage":
      return "usageSummary";
    default:
      return "daily";
  }
}

/** Whether a report lists projects, so the project redaction options apply. */
export function namesProjects(preset: ExportPreset): boolean {
  return preset === "byProject";
}

/** Paths are left out whenever names are hashed: a path names its project. */
export function pathsLeftOut(redaction: ExportRedaction): boolean {
  return redaction.dropPaths || redaction.hashProjectNames;
}
