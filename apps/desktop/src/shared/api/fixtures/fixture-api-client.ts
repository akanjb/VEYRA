/**
 * An {@link ApiClient} backed by the synthetic dataset, for development in a plain
 * browser and for UI tests. It behaves like the core: sources start disabled until the
 * user consents, rescans emit `data-changed`, and errors use the same typed union.
 */
import type {
  Capability,
  CommandName,
  EventMap,
  EventName,
  SessionDetail,
  SourceSummary,
} from "@veyra/contracts";

import type { ApiClient, CommandRequest, CommandResponse } from "../api-client";
import { ApiCallError } from "../api-errors";
import { sessionRow, tokensOf, type Flat } from "./aggregate";
import { FIXTURE_PROJECTS, fixtureLabel, generateSessions } from "./dataset";
import { activity } from "./activity-view";
import { FixtureBilling } from "./billing-view";
import { cache } from "./cache-view";
import { costView } from "./cost-view";
import { exportPreview } from "./export-view";
import { FixtureStorage } from "./storage-view";
import { models } from "./models-view";
import { costAssumptions } from "./pricing-view";
import { FixtureProjects } from "./projects-view";
import { tools } from "./tools-view";
import { usage } from "./usage-view";
import { whatIf } from "./what-if-view";
import { overview, sessions } from "./views";

/** What the Claude Code connector reports (as its descriptor in the core says). */
const CAPABILITIES: Capability[] = [
  { label: "Tokens", support: "full", note: null },
  { label: "Cache", support: "full", note: null },
  {
    label: "Reasoning",
    support: "partial",
    note: "Counted within output; not reported separately",
  },
  { label: "Tools", support: "full", note: null },
  { label: "Sessions", support: "full", note: null },
  { label: "Projects", support: "full", note: null },
];

/** The other tools of this build, none of which the fixture device has. */
const NOT_FOUND = ["Codex", "Gemini CLI", "OpenCode", "GitHub Copilot CLI"];

export interface FixtureOptions {
  /** Start like a first run: the source is detected but not yet enabled. */
  fresh?: boolean;
  now?: number;
}

class FixtureState {
  private readonly all;
  private enabled: boolean;
  private consentedAt: number | null;
  private readonly listeners = new Map<string, Set<(payload: never) => void>>();

  constructor(private readonly options: Required<FixtureOptions>) {
    this.all = generateSessions(options.now);
    this.enabled = !options.fresh;
    this.consentedAt = options.fresh ? null : options.now - 90 * 86_400_000;
  }

  readonly projects = new FixtureProjects();
  readonly billing = new FixtureBilling();
  readonly storage = new FixtureStorage();

  get now(): number {
    return this.options.now;
  }

  /** Every event with its session, counted toward the project it is merged into. */
  flat(): Flat[] {
    if (!this.enabled) return [];
    return this.all.flatMap((original) => {
      const projectId = this.projects.effective(original.projectId);
      const session = projectId === original.projectId ? original : { ...original, projectId };
      return session.events.map((event) => ({ session, event }));
    });
  }

  source(): SourceSummary {
    const events = this.enabled ? this.all.reduce((n, s) => n + s.events.length, 0) : 0;
    return {
      id: 1,
      connectorId: "claude-code",
      displayName: "Claude Code",
      root: "C:\\Users\\dev\\.claude\\projects",
      enabled: this.enabled,
      documentedFormat: false,
      consentedAt: this.consentedAt,
      lastScanAt: this.enabled ? this.options.now - 12_000 : null,
      lastSuccessAt: this.enabled ? this.options.now - 12_000 : null,
      lastError: null,
      filesActive: this.enabled ? 214 : 0,
      filesMissing: 0,
      lastIngestedAt: this.enabled ? this.options.now - 12_000 : null,
      events,
      recentIssues: this.enabled ? [{ kind: "malformed-json", occurrences: 2 }] : [],
      capabilities: CAPABILITIES,
    };
  }

  setEnabled(enabled: boolean): SourceSummary {
    this.enabled = enabled;
    if (enabled) this.consentedAt ??= Date.now();
    setTimeout(() => {
      this.emit("data-changed", { inserted: enabled ? 1 : 0, updated: 0 });
    }, 300);
    return this.source();
  }

  detail(id: number): SessionDetail {
    const session = this.enabled ? this.all.find((s) => s.id === id) : undefined;
    if (session === undefined) {
      throw new ApiCallError("get_session", {
        kind: "notFound",
        message: `session ${String(id)} does not exist`,
      });
    }
    const children = this.all.filter((s) => s.parentId === id);
    return {
      session: sessionRow(session, session.events, FIXTURE_PROJECTS),
      events: session.events.map((e) => ({
        at: e.at,
        provider: e.provider,
        model: fixtureLabel(e.model),
        tokens: {
          input: e.input,
          cacheRead: e.cacheRead,
          cacheWrite: e.cacheWrite,
          output: e.output,
          reasoning: e.reasoning,
          total: tokensOf(e),
        },
        costNanoUsd: e.costNanoUsd,
        accuracy: "exact",
        tools: e.tools,
      })),
      subagents: children.map((c) => sessionRow(c, c.events, FIXTURE_PROJECTS)),
    };
  }

  on<E extends EventName>(event: E, handler: (payload: EventMap[E]) => void): () => void {
    const set = this.listeners.get(event) ?? new Set();
    set.add(handler);
    this.listeners.set(event, set);
    return () => set.delete(handler);
  }

  emit<E extends EventName>(event: E, payload: EventMap[E]): void {
    for (const handler of this.listeners.get(event) ?? [])
      (handler as (p: EventMap[E]) => void)(payload);
  }
}

type Handlers = { [C in CommandName]: (request: CommandRequest<C>) => CommandResponse<C> };

type BillingCommand =
  "get_billing" | "save_billing_entry" | "delete_billing_entry" | "save_budget" | "delete_budget";

function billingHandlers(billing: FixtureBilling): Pick<Handlers, BillingCommand> {
  return {
    get_billing: () => billing.settings(),
    save_billing_entry: (r) => billing.saveEntry(r.id, r.entry),
    delete_billing_entry: (r) => {
      billing.deleteEntry(r.id);
      return {};
    },
    save_budget: (r) => billing.saveBudget(r.id, r.budget),
    delete_budget: (r) => {
      billing.deleteBudget(r.id);
      return {};
    },
  };
}

function exportHandlers(state: FixtureState): Pick<Handlers, "preview_export" | "save_export"> {
  return {
    preview_export: (r) => exportPreview(state.flat(), r),
    // There is no file system in the browser: report what would have been saved.
    save_export: (r) => ({ saved: true, rowCount: exportPreview(state.flat(), r).rowCount }),
  };
}

type StorageCommand = "get_storage" | "create_backup" | "preview_restore" | "restore_backup";

function storageHandlers(state: FixtureState): Pick<Handlers, StorageCommand> {
  return {
    get_storage: (r) => state.storage.overview(state.flat(), r.timeZone),
    create_backup: () => state.storage.create(state.flat(), Date.now()),
    preview_restore: (r) => state.storage.preview(state.flat(), r.name),
    // A browser cannot restart Veyra; the restore is only checked.
    restore_backup: (r) => {
      state.storage.preview(state.flat(), r.name);
      return {};
    },
  };
}

function projectChanges(
  state: FixtureState,
): Pick<Handlers, "rename_project" | "merge_project" | "unmerge_project"> {
  return {
    rename_project: (r) => state.projects.rename(r.id, r.name),
    merge_project: (r) => {
      state.projects.merge(r.from, r.into);
      return {};
    },
    unmerge_project: (r) => {
      state.projects.unmerge(r.id);
      return {};
    },
  };
}

/** Creates the fixture client. */
export function createFixtureApiClient(options: FixtureOptions = {}): ApiClient {
  const state = new FixtureState({ fresh: options.fresh ?? false, now: options.now ?? Date.now() });
  const handlers: Handlers = {
    get_app_info: () => ({
      productName: "Veyra",
      version: "0.1.0",
      buildProfile: "debug",
      os: "fixture",
      arch: "web",
    }),
    list_sources: () => ({ sources: [state.source()], notFound: NOT_FOUND }),
    set_source_enabled: (r) => state.setEnabled(r.enabled),
    request_rescan: () => {
      setTimeout(() => {
        state.emit("data-changed", { inserted: 0, updated: 0 });
      }, 300);
      return {};
    },
    get_overview: (r) => overview(state.flat(), r.scope),
    get_usage: (r) => usage(state.flat(), r.scope),
    get_cost: (r) => costView(state.flat(), r.scope, state.billing, state.now),
    get_cost_assumptions: (r) => costAssumptions(state.flat(), r.scope, state.now),
    get_what_if: (r) => whatIf(state.flat(), r.scope, r.model),
    get_cache: (r) => cache(state.flat(), r.scope),
    get_models: (r) => models(state.flat(), r.scope),
    get_activity: (r) => activity(state.flat(), r.scope),
    get_tools: (r) => tools(state.flat(), r.scope),
    list_projects: (r) => state.projects.list(state.flat(), r.scope),
    get_project: (r) => state.projects.detail(state.flat(), r.scope, r.id),
    ...projectChanges(state),
    ...billingHandlers(state.billing),
    ...exportHandlers(state),
    ...storageHandlers(state),
    list_sessions: (r) => sessions(state.flat(), r),
    get_session: (r) => state.detail(r.id),
  };
  return {
    call(command, request) {
      // Resolve asynchronously, like IPC, so loading states are exercised.
      return new Promise((resolve, reject) => {
        setTimeout(() => {
          try {
            resolve(
              (handlers[command] as (r: typeof request) => CommandResponse<typeof command>)(
                request,
              ),
            );
          } catch (error) {
            reject(error instanceof Error ? error : new Error(String(error)));
          }
        }, 60);
      });
    },
    subscribe: (event, handler) => state.on(event, handler),
  };
}
