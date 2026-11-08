/**
 * A deterministic synthetic dataset for development and UI tests: about 120 days of
 * Claude Code sessions with subagents and tool calls. Never shipped in production builds
 * and never derived from real logs.
 */
import type { Labelled } from "@veyra/contracts";

export interface FixtureTool {
  name: string;
  mcpServer: string | null;
  isError: boolean | null;
}

export interface FixtureEvent {
  at: number;
  model: string;
  provider: string;
  input: number;
  cacheRead: number;
  cacheWrite: number;
  output: number;
  reasoning: number;
  costNanoUsd: number;
  tools: FixtureTool[];
}

export interface FixtureSession {
  id: number;
  key: string;
  app: string;
  interface: string;
  projectId: number;
  parentId: number | null;
  events: FixtureEvent[];
}

export const FIXTURE_PROJECTS: readonly string[] = [
  "c:/users/dev/code/veyra",
  "c:/users/dev/code/storefront",
  "c:/users/dev/code/ml-pipeline",
  "/home/dev/infra",
  "/home/dev/docs-site",
  "/home/dev/scratch",
];

/** Model id, provider, display name, $/MTok input, output, cache read, cache write. */
export const FIXTURE_MODELS = [
  ["claude-opus-5-5", "anthropic", "Claude Opus 5.5", 4, 20, 0.2, 5],
  ["claude-sonnet-5-5", "anthropic", "Claude Sonnet 5.5", 2, 10, 0.1, 2.5],
  ["claude-haiku-5-5", "anthropic", "Claude Haiku 5.5", 0.5, 2.5, 0.05, 0.6],
] as const;

/** Display names, as the core's registry and connectors provide them. */
const FIXTURE_NAMES: ReadonlyMap<string, string> = new Map<string, string>([
  ["anthropic", "Anthropic"],
  ["claude-code", "Claude Code"],
  ...FIXTURE_MODELS.map(([id, , name]): [string, string] => [id, name]),
]);

/** An identifier with its display name (itself when unknown, like the core). */
export function fixtureLabel(key: string): Labelled {
  return { key, label: FIXTURE_NAMES.get(key) ?? key };
}

const TOOLS: readonly FixtureTool[] = [
  { name: "Read", mcpServer: null, isError: false },
  { name: "Edit", mcpServer: null, isError: false },
  { name: "Bash", mcpServer: null, isError: false },
  { name: "Grep", mcpServer: null, isError: false },
  { name: "Bash", mcpServer: null, isError: true },
  { name: "mcp__github__search_code", mcpServer: "github", isError: false },
];

/** A UUID-shaped id derived from a number, like the session ids real logs use. */
function uuidLike(n: number): string {
  const hex = (Math.imul(n, 2_654_435_761) >>> 0).toString(16).padStart(8, "0");
  return `${hex}-0000-4000-8000-${hex}0000`;
}

/** mulberry32: small, fast, deterministic. */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4_294_967_296;
  };
}

const pick = <T>(items: readonly T[], r: number): T => items[Math.floor(r * items.length)] as T;

function makeEvent(rand: () => number, at: number, modelIndex: number): FixtureEvent {
  const [model, provider, , inRate, outRate, readRate, writeRate] =
    FIXTURE_MODELS[modelIndex] ?? FIXTURE_MODELS[1];
  const input = Math.round(2 + rand() * 40);
  const cacheRead = Math.round(8_000 + rand() * 60_000);
  const cacheWrite = Math.round(rand() * 4_000);
  const output = Math.round(50 + rand() * 1_500);
  const reasoning = Math.round(output * rand() * 0.4);
  const cost =
    (input * inRate + output * outRate + cacheRead * readRate + cacheWrite * writeRate) * 1_000;
  const toolCount = Math.floor(rand() * 3);
  return {
    at,
    model,
    provider,
    input,
    cacheRead,
    cacheWrite,
    output,
    reasoning,
    costNanoUsd: Math.round(cost),
    tools: Array.from({ length: toolCount }, () => pick(TOOLS, rand())),
  };
}

/** Sessions ending at `now`, spread over `days` days, with typical working hours. */
export function generateSessions(now: number, days = 120, seed = 7): FixtureSession[] {
  const rand = random(seed);
  const sessions: FixtureSession[] = [];
  const dayMs = 86_400_000;
  const start = Math.floor(now / dayMs) * dayMs - (days - 1) * dayMs;
  for (let d = 0; d < days; d += 1) {
    const weekend = new Date(start + d * dayMs).getUTCDay() % 6 === 0;
    const count = Math.floor(rand() * (weekend ? 2 : 5));
    for (let s = 0; s < count; s += 1) {
      const begin = start + d * dayMs + (3 + rand() * 12) * 3_600_000;
      if (begin > now) continue;
      const modelIndex = rand() < 0.55 ? 0 : rand() < 0.8 ? 1 : 2;
      const id = sessions.length + 1;
      const events: FixtureEvent[] = [];
      let at = begin;
      const length = 5 + Math.floor(rand() * 45);
      for (let e = 0; e < length && at <= now; e += 1) {
        events.push(makeEvent(rand, at, modelIndex));
        at += (10 + rand() * (rand() < 0.1 ? 1_800 : 140)) * 1_000;
      }
      const projectId = 1 + Math.floor(rand() * rand() * FIXTURE_PROJECTS.length);
      const base = { app: "claude-code", interface: rand() < 0.7 ? "desktop" : "cli", projectId };
      sessions.push({
        id,
        key: `claude-code:${uuidLike(id)}`,
        parentId: null,
        events,
        ...base,
      });
      if (rand() < 0.35 && events.length > 3) {
        const childEvents = events.slice(1, 4).map((e) => makeEvent(rand, e.at + 5_000, 2));
        sessions.push({
          id: sessions.length + 1,
          key: `claude-code:${uuidLike(id)}:agent-${String(id)}`,
          parentId: id,
          events: childEvents,
          ...base,
        });
      }
    }
  }
  return sessions;
}
