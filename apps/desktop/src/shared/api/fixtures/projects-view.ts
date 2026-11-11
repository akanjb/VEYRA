/**
 * Projects for the synthetic dataset: names and merges kept in memory, and responses
 * shaped like the core's. Only for development and UI tests.
 */
import type { ProjectDetail, ProjectInfo, ProjectList, ProjectRow, Scope } from "@veyra/contracts";

import { ApiCallError } from "../api-errors";
import { activity, localDay, measuresOf, rank, select, tokensOf, type Flat } from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel } from "./dataset";
import { daysOf, groupBy, overview, sessions } from "./views";

export class FixtureProjects {
  private readonly names = new Map<number, string>();
  private readonly merged = new Map<number, number>();

  /** The project a project's usage counts toward. */
  effective(id: number): number {
    return this.merged.get(id) ?? id;
  }

  private check(id: number, command: string): void {
    if (FIXTURE_PROJECTS[id - 1] === undefined)
      throw new ApiCallError(command, {
        kind: "notFound",
        message: `project ${String(id)} does not exist`,
      });
  }

  info(id: number): ProjectInfo {
    const path = FIXTURE_PROJECTS[id - 1] ?? "";
    const name = this.names.get(id) ?? null;
    return {
      id,
      path,
      name,
      label: name ?? path,
      aliases: [...this.merged.entries()]
        .filter(([, into]) => into === id)
        .map(([from]) => ({ id: from, path: FIXTURE_PROJECTS[from - 1] ?? "" })),
      mergedInto: this.merged.get(id) ?? null,
    };
  }

  rename(id: number, name: string | null): ProjectInfo {
    this.check(id, "rename_project");
    const trimmed = name?.trim() ?? "";
    if (trimmed === "") this.names.delete(id);
    else this.names.set(id, trimmed);
    return this.info(id);
  }

  merge(from: number, into: number): void {
    this.check(from, "merge_project");
    this.check(into, "merge_project");
    const target = this.effective(into);
    if (target === from)
      throw new ApiCallError("merge_project", {
        kind: "validation",
        message: "a project cannot be merged into itself",
      });
    for (const [id, to] of this.merged) if (to === from) this.merged.set(id, target);
    this.merged.set(from, target);
  }

  unmerge(id: number): void {
    this.check(id, "unmerge_project");
    this.merged.delete(id);
  }

  list(flat: readonly Flat[], scope: Scope): ProjectList {
    const selected = select(flat, scope, scope.start, scope.end);
    const dates = daysOf(scope.start, scope.end);
    const byProject = new Map<number, Flat[]>();
    for (const f of selected) {
      const list = byProject.get(f.session.projectId) ?? [];
      list.push(f);
      byProject.set(f.session.projectId, list);
    }
    const rows: ProjectRow[] = FIXTURE_PROJECTS.map((_, i) => i + 1)
      .filter((id) => !this.merged.has(id))
      .map((id) => {
        const events = byProject.get(id) ?? [];
        const act = activity(events);
        const perDay = new Map<string, number>();
        for (const f of events) {
          const day = localDay(f.event.at, scope.timeZone);
          perDay.set(day, (perDay.get(day) ?? 0) + tokensOf(f.event));
        }
        const models = groupBy(
          events,
          (f) => f.event.model,
          () => ({ label: "", provider: null }),
        );
        const top = [...models.entries()].sort(
          (a, b) => measuresOf(b[1].events).tokens - measuresOf(a[1].events).tokens,
        )[0];
        return {
          project: this.info(id),
          measures: measuresOf(events.map((f) => f.event)),
          sessions: act.sessions,
          activeMs: act.activeMs,
          topModel: top === undefined ? null : fixtureLabel(top[0]),
          lastActiveAt: events.length === 0 ? null : Math.max(...events.map((f) => f.event.at)),
          dailyTokens: dates.map((d) => perDay.get(d) ?? 0),
        };
      })
      .sort(
        (a, b) =>
          b.measures.tokens - a.measures.tokens || a.project.path.localeCompare(b.project.path),
      );
    return { dates, rows };
  }

  detail(flat: readonly Flat[], scope: Scope, id: number): ProjectDetail {
    this.check(id, "get_project");
    const own = flat.filter((f) => f.session.projectId === id);
    const summary = overview(own, scope);
    const selected = select(own, scope, scope.start, scope.end);
    const byProvider = groupBy(
      selected,
      (f) => f.event.provider,
      (f) => ({ label: fixtureLabel(f.event.provider).label, provider: null }),
    );
    const list = sessions(own, { scope, sort: "cost", page: 0, pageSize: 10 });
    return {
      project: this.info(id),
      kpis: summary.kpis,
      netCacheBenefitNanoUsd: {
        ...summary.kpis.costNanoUsd,
        definition: "Fixture: net cache benefit.",
      },
      composition: summary.composition,
      apps: summary.apps,
      daily: summary.daily,
      models: summary.models,
      providers: rank(byProvider, 8, measuresOf(selected.map((f) => f.event)).tokens),
      sessions: list.rows,
    };
  }
}
