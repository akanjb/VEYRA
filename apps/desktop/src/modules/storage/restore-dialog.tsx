/** Restoring a backup: what it holds against what is stored now, then confirm. */
import type { BackupInfo } from "@veyra/contracts";

import { useCommand, useCommandAction } from "@/shared/api";
import { fmt } from "@/shared/format";
import { Button, FormError, Modal } from "@/shared/ui";

function span(first: number | null, last: number | null): string {
  if (first === null || last === null) return "no events";
  return `${fmt.dateTime(first)} to ${fmt.dateTime(last)}`;
}

export function RestoreDialog({ backup, onClose }: { backup: BackupInfo; onClose: () => void }) {
  const preview = useCommand("preview_restore", { name: backup.name });
  const restore = useCommandAction("restore_backup");
  const data = preview.data;
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title="Restore backup"
      showTitle
    >
      <div className="grid gap-3 p-5 text-sm">
        <FormError error={preview.error} />
        {data === undefined ? (
          <p className="text-xs text-muted">Reading the backup…</p>
        ) : (
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1" data-numeric>
            <dt className="text-muted">Backup taken</dt>
            <dd>{fmt.dateTime(data.backup.createdAt)}</dd>
            <dt className="text-muted">It holds</dt>
            <dd>
              {fmt.integer(data.events)} events, {span(data.firstEventAt, data.lastEventAt)}
            </dd>
            <dt className="text-muted">Stored now</dt>
            <dd>{fmt.integer(data.currentEvents)} events</dd>
          </dl>
        )}
        <p className="text-xs text-muted">
          Veyra restarts and replaces its database with this backup. The current database is kept
          beside it until the next restore. Usage recorded after the backup is read again from your
          tools&apos; logs where they still exist.
        </p>
        <FormError error={restore.error} />
        {restore.isSuccess ? (
          <p role="status" className="text-xs text-muted">
            Restarting…
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            type="button"
            variant="primary"
            disabled={data === undefined || restore.isPending || restore.isSuccess}
            onClick={() => {
              restore.mutate({ name: backup.name });
            }}
          >
            Restore and restart
          </Button>
        </div>
      </div>
    </Modal>
  );
}
