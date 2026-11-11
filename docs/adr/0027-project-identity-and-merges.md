# 0027. Project identity, renaming and merging

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

A project is the working directory a source recorded usage in. The same real project often appears under several directories: a clone moved to another folder, a second worktree, or a path spelled differently by two tools. Paths are also long and hard to read.

The Projects page lets the user give a project a name and merge projects that are really one (03-ux.md §4). Both changes must reach every page at once, because every view reads `project_id` from the events and the 15-minute rollups (ADR-009). A merge must also be reversible. Users merge the wrong folder, and Veyra never deletes recorded usage.

## Decision

1. **A name is a label, not an identity.** `projects.display_name` holds the user's name, trimmed and at most 80 characters. An empty name clears it. The path stays the project's identity, and a renamed project is still found by its path. Renaming changes no usage and no rollup.

2. **Each event keeps the project it was recorded in.** `usage_events.source_project_id` is the recorded project and never changes after ingestion. `usage_events.project_id` is the effective project: the recorded one, or the project it is merged into. All reads and rollups keep using `project_id`, so no query changes.

3. **Merges are one level deep.** `projects.merged_into` points at the project that usage counts toward. A merge into a project that is itself merged goes to that project's target. When a project is merged, the projects already merged into it move with it. Following `merged_into` once therefore always reaches an unmerged project.

4. **Merging and unmerging rewrite the effective project in one transaction.** The store sets `project_id` for every event recorded in the moved projects and rebuilds the rollups of the buckets those events fall in. All of it is one write transaction, followed by a single `data-changed` notification. Unmerging sets a project's events back to their recorded project.

5. **New events follow existing merges.** Ingestion resolves each event's effective project when it writes it, so usage recorded after a merge counts toward the target without another rebuild.

6. **Refusals are typed.** Merging a project into itself or into a project merged into it, and unmerging a project that is not merged, are refused as validation errors. An unknown project is refused as not found. Nothing is written in either case.

## Alternatives considered

- **Resolve merges at read time with a join to `projects`.** No rewrite, but every query and every rollup would need the join. Rollups are keyed by `project_id`, so they would have to be rebuilt per read or keyed by the recorded project and re-grouped on every page. Rewriting once is cheaper and keeps reads unchanged.
- **Rewrite `project_id` without keeping the recorded project.** Simpler, but a merge could not be undone without re-reading every source, and sources may have deleted their files since.
- **Chains of merges.** Following an arbitrary chain on every write and read adds cycle checks and lookups for no visible benefit. Flattening keeps one hop.
- **A separate alias table.** It would duplicate `merged_into` and add another place to keep in sync.

## Consequences

- A merge costs one indexed update of the moved projects' events and a rebuild of their buckets. For a large project this is a noticeable write, but it is a rare, user-started action.
- Unmerging a project that was merged together with others restores only that project. The others stay merged into the target.
- The event table has one more column and one partial index.
- Every page and session detail shows the effective project. The recorded path stays visible as an alias of the target, and is searchable on the Projects page.

## Revisit when

Users need to split one directory into several projects, or to merge by rule (for example every worktree of a repository). Either needs a mapping rule rather than single merges.
