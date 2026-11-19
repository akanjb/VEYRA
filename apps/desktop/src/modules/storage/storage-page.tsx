/** The Storage page: Veyra's own database, what it holds, and its backups (ADR-033). */
import type { BackupInfo, MonthEvents, StorageOverview, TableBytes } from "@veyra/contracts";
import { useState } from "react";

import { useCommand, useCommandMutation } from "@/shared/api";
import { DataTable } from "@/shared/charts";
import { systemTimeZone } from "@/shared/filters";
import { fmt } from "@/shared/format";
import { Button, Card, ErrorState, FormError, Skeleton, Stat } from "@/shared/ui";

import { DeleteData } from "./delete-data";
import { RestoreDialog } from "./restore-dialog";

function Footprint({ storage }: { storage: StorageOverview }) {
  return (
    <Card title="Database">
      <dl className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <Stat label="Size on disk" value={fmt.bytes(storage.databaseBytes)} />
        <Stat label="Events" value={fmt.integer(storage.events)} />
        <Stat
          label="First event"
          value={storage.firstEventAt === null ? fmt.MISSING : fmt.dateTime(storage.firstEventAt)}
        />
        <Stat
          label="Backups"
          value={`${String(storage.backups.length)} of ${String(storage.keptBackups)}`}
        />
      </dl>
    </Card>
  );
}

function Backups({ storage }: { storage: StorageOverview }) {
  const create = useCommandMutation("create_backup");
  const [restoring, setRestoring] = useState<BackupInfo | null>(null);
  return (
    <Card title="Backups">
      <div className="grid gap-3">
        <p className="text-xs text-muted">
          Veyra backs up its database once a day and keeps the newest {String(storage.keptBackups)}.
          Your tools&apos; own files are never touched.
        </p>
        {storage.backups.length === 0 ? (
          <p className="text-sm text-muted">No backups yet.</p>
        ) : (
          <ul className="grid gap-2" aria-label="Backups">
            {storage.backups.map((b) => (
              <li key={b.name} className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
                <span data-numeric>{fmt.dateTime(b.createdAt)}</span>
                <span className="text-muted" data-numeric>
                  {fmt.bytes(b.bytes)}
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto"
                  aria-label={`Restore the backup of ${fmt.dateTime(b.createdAt)}`}
                  onClick={() => {
                    setRestoring(b);
                  }}
                >
                  Restore…
                </Button>
              </li>
            ))}
          </ul>
        )}
        <FormError error={create.error} />
        <div>
          <Button
            size="sm"
            disabled={create.isPending}
            onClick={() => {
              create.mutate({});
            }}
          >
            {create.isPending ? "Backing up…" : "Back up now"}
          </Button>
        </div>
      </div>
      {restoring === null ? null : (
        <RestoreDialog
          backup={restoring}
          onClose={() => {
            setRestoring(null);
          }}
        />
      )}
    </Card>
  );
}

export function StoragePage() {
  const storage = useCommand("get_storage", { timeZone: systemTimeZone() });
  if (storage.isPending) return <Skeleton className="h-80" />;
  if (storage.isError)
    return <ErrorState error={storage.error} onRetry={() => void storage.refetch()} />;
  const data = storage.data;
  return (
    <div className="grid max-w-5xl gap-4">
      <Footprint storage={data} />
      <Backups storage={data} />
      <DeleteData />
      <div className="grid gap-4 xl:grid-cols-2">
        <Card title="Events per month">
          <DataTable<MonthEvents>
            caption="Events recorded per month"
            columns={[
              { header: "Month", cell: (m) => fmt.month(m.month) },
              { header: "Events", cell: (m) => fmt.integer(m.events), numeric: true },
            ]}
            rows={[...data.months].reverse()}
            rowKey={(m) => m.month}
          />
        </Card>
        <Card title="Space per table">
          <DataTable<TableBytes>
            caption="Space each table takes, with its indexes"
            columns={[
              { header: "Table", cell: (t) => <span className="break-all">{t.table}</span> },
              { header: "Size", cell: (t) => fmt.bytes(t.bytes), numeric: true },
            ]}
            rows={data.tables}
            rowKey={(t) => t.table}
          />
        </Card>
      </div>
    </div>
  );
}
