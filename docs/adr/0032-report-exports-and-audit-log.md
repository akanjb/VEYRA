# 0032. Report exports, stored settings and the audit log

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

The PRD asks for reports the user can keep or hand to a client (UC-08, FR-EXP-01 to 04):
- CSV, JSON and XLSX;
- presets: usage and cost summaries, per project, per model, and daily, weekly and monthly reports;
- an accuracy column and a cost-kind column;
- redaction of project names, paths, session titles and device names.

The acceptance criteria require:
- a report's totals equal the on-screen totals for the same filters;
- verified redaction: hashed names, and no paths or titles when they are turned off.

The threat model (architecture §11) allows writes only through the native save dialog, never to a path the dashboard names. The architecture also plans a `settings` table and an `audit_log` that records exports without personal data.

## Decision

1. **Reports come from the analytics engine.** `views::export` builds every report with the same queries as the pages, over the whole scope, with no top-N cut. Its rows therefore add up to the Overview's totals; a test checks every preset.
   - **Columns:** each row has the key columns of its preset, then the registry's token measures and requests (with the registry's keys and labels), then the API-equivalent cost, the cost kind and the token accuracy.
   - **Cost kinds:** the cost summary lists recorded subscriptions and charges (US dollars only, as on the Cost page) as rows of their own cost kind. Reports have no total rows, so cost kinds are never added (ADR-004).

2. **Writers are format-independent.** A report is a table of typed cells.
   - **CSV:** follows RFC 4180 with CRLF line ends. A variant starts with a byte-order mark, so Excel reads it as UTF-8. Amounts are dollars with nine decimals, so they are exact. Text that a spreadsheet would run as a formula (starting with `=`, `+`, `-`, `@`, a tab or a carriage return) gets an apostrophe in front, because project names come from folder names anyone can choose.
   - **JSON:** rows are objects keyed by stable column keys, with amounts as integer billionths of a dollar.
   - **XLSX:** one worksheet with a bold, frozen header row. Counts and amounts are numbers, and amounts show nine decimals. Text is stored as text, so a spreadsheet never runs it as a formula.

3. **Redaction.**
   - **Hashed names:** a project's name becomes `project-` and 12 hex digits of SHA-256 over a secret salt and the project's path. The salt is 32 random bytes, created once per device and kept in `settings`. The same project therefore gets the same name in every report from this device, and nobody who receives a report can guess names back.
   - **Paths:** a hashed report never includes paths, since a path names its project. Paths can also be left out on their own.
   - **Titles and devices:** reports never contain session titles or device names, so those options are not offered.

4. **Only the core writes the file.** `save_export` builds the file, then calls a `FileSaver` port. The desktop shell implements it with the official Tauri dialog plugin's Rust API: the user picks the location in the operating system's save dialog, and the core writes there. The window is granted no dialog command, and a test checks that the dashboard cannot open the dialog itself. `preview_export` returns the first 20 rows exactly as they will be written, the row count and the suggested file name.

5. **Settings and the audit log** (migration 0008).
   - `settings` holds JSON values under stable keys. `setting_or_init` keeps the first value stored, so two first exports cannot create two salts.
   - `audit_log` records who did what and when. Each saved export adds an entry such as "per-project report, CSV, 12 rows, project names hashed, paths left out", never the file's name or location. Previews are not audited.
   - Neither table changes usage data, so writing them does not refresh the dashboard.

6. **Dependency admission** (CONTRIBUTING checklist):

   | Crate | Version (published) | Licence | Why |
   |---|---|---|---|
   | `tauri-plugin-dialog` | 2.8.4 (2026-10-28) | Apache-2.0 OR MIT | The native save dialog. Maintained by the Tauri team in `tauri-apps/plugins-workspace`; no open advisories. |
   | `rfd`, `tauri-plugin-fs`, `tauri-plugin` | 0.17.2, 2.7.1, 2.7.2 | MIT; Apache-2.0 OR MIT | Its dependencies; the only new crates in the lockfile. |
   | `sha2` 0.10.9, `getrandom` 0.3.4 | already in the lockfile through Tauri | MIT OR Apache-2.0 | Hashing and the salt; now direct dependencies of `veyra-app`. |
   | `rust_xlsxwriter` | 0.102.0 (2026-10-29), default features off | MIT OR Apache-2.0 | XLSX writing. One maintainer with a long record (the author of the Python and C XlsxWriter libraries); frequent releases; no advisories. |
   | `zip` 9.3.0, `zopfli` 0.8.4, `typed-path` 0.13.0, `nix` 0.31.3 (Unix only), `cfg_aliases` 0.2.2 | 2026-11-03 or earlier | MIT; Apache-2.0; MIT OR Apache-2.0 | Its dependencies. `zip` publishes almost daily, so it is pinned to the newest release at least seven days old. |

   Without the plugin, the shell would need its own bindings to each operating system's dialog (`rfd` directly), and the plugin is the maintained, reviewed route. Without `rust_xlsxwriter`, XLSX would need a hand-written Office Open XML writer, which is a large surface to get right.

## Alternatives considered

- **Building reports from the page responses.** Pages cut rankings to the top N and an "Other" row, so their rows would not list every project or model.
- **A download through the webview.** Tauri's webview does not offer the browser download flow, and a dashboard-chosen path is what the threat model rules out.
- **Unsalted hashes, or numbering projects in each report.** Unsalted hashes of short folder names can be guessed by hashing likely names. Numbering per report changes from one report to the next, so a client could not follow a project across monthly reports.
- **Recording the file name in the audit log.** It can name a client or the user; the audit log describes the action only.

## Consequences

- One engine serves both pages and reports, so a report can never disagree with the screen.
- The settings table is ready for the Settings page (time zone, titles policy and more).
- The dialog plugin adds four crates; its commands stay unreachable from the window.

## Revisit when

Reports need to cover sessions or devices: the redaction options for titles and device names then apply.
