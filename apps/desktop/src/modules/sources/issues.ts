/**
 * Plain-language names for the problem kinds the ingest pipeline and connectors report.
 * Kinds are stable identifiers; one without a name here is shown as its identifier.
 */
const ISSUE_LABELS: Readonly<Record<string, string>> = {
  "malformed-json": "Lines that are not valid JSON",
  "invalid-record": "Records missing required fields",
  "oversize-line": "Lines too long to read safely",
  "unreadable-file": "Files that could not be opened",
};

export function issueLabel(kind: string): string {
  return ISSUE_LABELS[kind] ?? kind;
}
