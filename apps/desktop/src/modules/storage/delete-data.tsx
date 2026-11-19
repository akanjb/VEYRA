/** Deleting Veyra's own stored usage by application and dates, after a preview (FR-STO-04). */
import type { DeleteDataRequest, DeletePreview } from "@veyra/contracts";
import { useState } from "react";

import { useCommand, useCommandMutation } from "@/shared/api";
import { systemTimeZone } from "@/shared/filters";
import { fmt } from "@/shared/format";
import { Button, Card, FormError, Modal, SelectField, TextField } from "@/shared/ui";

const ALL = "";

function span(preview: DeletePreview): string {
  const { firstEventAt: first, lastEventAt: last } = preview;
  if (first === null || last === null) return "";
  return `, ${fmt.dateTime(first)} to ${fmt.dateTime(last)}`;
}

function ConfirmDelete({
  request,
  preview,
  onClose,
}: {
  request: DeleteDataRequest;
  preview: DeletePreview;
  onClose: () => void;
}) {
  const remove = useCommandMutation("delete_data");
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title="Delete stored usage"
      showTitle
    >
      <div className="grid gap-3 p-5 text-sm">
        <p data-numeric>
          {fmt.integer(preview.events)} events{span(preview)} will be deleted from Veyra.
        </p>
        <p className="text-xs text-muted">
          A backup is taken first, so this can be undone by restoring it. Your tools&apos; own files
          are never touched. Lines Veyra has already read are not read again unless a file is
          replaced.
        </p>
        <FormError error={remove.error} />
        {remove.isSuccess ? (
          <p role="status" className="text-xs text-muted" data-numeric>
            Deleted {fmt.integer(remove.data.events)} events.
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            {remove.isSuccess ? "Close" : "Cancel"}
          </Button>
          {remove.isSuccess ? null : (
            <Button
              type="button"
              variant="primary"
              disabled={remove.isPending}
              onClick={() => {
                remove.mutate(request);
              }}
            >
              {remove.isPending ? "Deleting…" : "Back up and delete"}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}

/** The application and the optional first and last days of a deletion. */
function ScopeFields({
  connector,
  start,
  end,
  onConnector,
  onStart,
  onEnd,
}: {
  connector: string;
  start: string;
  end: string;
  onConnector: (value: string) => void;
  onStart: (value: string) => void;
  onEnd: (value: string) => void;
}) {
  const sources = useCommand("list_sources", {});
  const apps = new Map((sources.data?.sources ?? []).map((s) => [s.connectorId, s.displayName]));
  const options = [
    { value: ALL, label: "All applications" },
    ...[...apps].map(([value, label]) => ({ value, label })),
  ];
  return (
    <div className="grid gap-3 sm:grid-cols-3">
      <SelectField label="Application" value={connector} options={options} onChange={onConnector} />
      <TextField label="From (optional)" type="date" value={start} onChange={onStart} />
      <TextField label="To (optional)" type="date" value={end} onChange={onEnd} />
    </div>
  );
}

function matchText(preview: DeletePreview | undefined): string {
  if (preview === undefined) return "Counting…";
  if (preview.events === 0) return "Nothing stored matches.";
  return `${fmt.integer(preview.events)} events match${span(preview)}.`;
}

export function DeleteData() {
  const [connector, setConnector] = useState(ALL);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  // What was shown when Delete was pressed, kept while the counts refresh afterwards.
  const [confirming, setConfirming] = useState<{
    request: DeleteDataRequest;
    preview: DeletePreview;
  } | null>(null);
  const request: DeleteDataRequest = {
    timeZone: systemTimeZone(),
    start: start === "" ? null : start,
    end: end === "" ? null : end,
    connectorId: connector === ALL ? null : connector,
  };
  const preview = useCommand("preview_delete", request);
  const data = preview.data;
  return (
    <Card title="Delete stored usage">
      <div className="grid gap-3">
        <ScopeFields
          connector={connector}
          start={start}
          end={end}
          onConnector={setConnector}
          onStart={setStart}
          onEnd={setEnd}
        />
        <FormError error={preview.error} />
        <div className="flex flex-wrap items-center gap-3">
          <p className="text-sm text-muted" role="status" data-numeric>
            {matchText(data)}
          </p>
          <Button
            size="sm"
            className="ml-auto"
            disabled={data === undefined || data.events === 0 || preview.isError}
            onClick={() => {
              if (data !== undefined) setConfirming({ request, preview: data });
            }}
          >
            Delete…
          </Button>
        </div>
      </div>
      {confirming === null ? null : (
        <ConfirmDelete
          {...confirming}
          onClose={() => {
            setConfirming(null);
          }}
        />
      )}
    </Card>
  );
}
