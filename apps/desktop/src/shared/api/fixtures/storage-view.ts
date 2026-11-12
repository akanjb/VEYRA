/**
 * Storage over the synthetic dataset, shaped like the core's `get_storage` and backup
 * commands. Sizes are illustrative; the core measures the real database.
 */
import type { BackupInfo, RestorePreview, StorageOverview } from "@veyra/contracts";

import { ApiCallError } from "../api-errors";
import { localDay, type Flat } from "./aggregate";

const KEPT = 7;
const BYTES_PER_EVENT = 900;

export class FixtureStorage {
  private backups: BackupInfo[] = [];

  overview(flat: readonly Flat[], timeZone: string): StorageOverview {
    const times = flat.map((f) => f.event.at).sort((a, b) => a - b);
    const counts = new Map<string, number>();
    for (const at of times) {
      const month = localDay(at, timeZone).slice(0, 7);
      counts.set(month, (counts.get(month) ?? 0) + 1);
    }
    const events = times.length;
    return {
      databaseBytes: events * BYTES_PER_EVENT,
      tables: [
        { table: "usage_events", bytes: events * 520 },
        { table: "rollup_15m", bytes: events * 210 },
        { table: "event_costs", bytes: events * 170 },
      ],
      events,
      firstEventAt: times[0] ?? null,
      lastEventAt: times.at(-1) ?? null,
      months: [...counts].map(([month, n]) => ({ month, events: n })),
      backups: this.backups,
      keptBackups: KEPT,
    };
  }

  create(flat: readonly Flat[], now: number): BackupInfo {
    const backup = {
      name: `veyra-${String(now)}.db`,
      createdAt: now,
      bytes: flat.length * BYTES_PER_EVENT,
    };
    this.backups = [backup, ...this.backups].slice(0, KEPT);
    return backup;
  }

  preview(flat: readonly Flat[], name: string): RestorePreview {
    const backup = this.backups.find((b) => b.name === name);
    if (backup === undefined) {
      throw new ApiCallError("preview_restore", {
        kind: "notFound",
        message: `no backup is named \`${name}\``,
      });
    }
    const times = flat.map((f) => f.event.at).filter((at) => at <= backup.createdAt);
    return {
      backup,
      events: times.length,
      firstEventAt: times.length === 0 ? null : Math.min(...times),
      lastEventAt: times.length === 0 ? null : Math.max(...times),
      currentEvents: flat.length,
    };
  }
}
