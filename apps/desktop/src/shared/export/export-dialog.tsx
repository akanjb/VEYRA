/**
 * Export a report of the current scope (ADR-032): choose a report, a format and what to
 * leave out, preview the rows exactly as written, then save through the operating
 * system's save dialog.
 */
import type { ExportFormat, ExportPreset, ExportRedaction, ExportRequest } from "@veyra/contracts";
import { useState } from "react";

import { useCommand, useCommandAction } from "../api";
import { useFilters } from "../filters";
import { Button, FormError, Modal, Segmented, SelectField, Switch } from "../ui";

import { FORMATS, PRESETS, namesProjects, pathsLeftOut } from "./export-model";

function Toggle({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-3 text-sm">
      {label}
      <Switch
        label={label}
        checked={checked}
        disabled={disabled ?? false}
        onCheckedChange={onChange}
      />
    </label>
  );
}

function Preview({ request }: { request: ExportRequest }) {
  const preview = useCommand("preview_export", request);
  if (preview.error !== null) return <FormError error={preview.error} />;
  const data = preview.data;
  if (data === undefined) return <p className="text-xs text-muted">Preparing the preview…</p>;
  return (
    <div className="grid gap-1.5">
      <p className="text-xs text-muted" data-numeric>
        {data.rowCount === 0
          ? "No rows: nothing was recorded in this period."
          : `Showing ${String(data.rows.length)} of ${String(data.rowCount)} rows · ${data.fileName}`}
      </p>
      {data.rowCount === 0 ? null : (
        <div
          className="max-h-56 overflow-auto rounded-control border border-border"
          tabIndex={0}
          role="region"
          aria-label="Report preview"
        >
          <table className="min-w-full text-xs" data-numeric>
            <thead className="sticky top-0 bg-sunken">
              <tr>
                {data.columns.map((column) => (
                  <th
                    key={column}
                    scope="col"
                    className="px-2 py-1 text-left font-medium whitespace-nowrap"
                  >
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.rows.map((row, r) => (
                <tr key={r} className="border-t border-border">
                  {row.map((cell, c) => (
                    <td key={c} className="px-2 py-1 whitespace-nowrap">
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function Privacy({
  redaction,
  onChange,
}: {
  redaction: ExportRedaction;
  onChange: (redaction: ExportRedaction) => void;
}) {
  return (
    <fieldset className="grid gap-2">
      <legend className="mb-1 text-xs font-medium text-muted">Privacy</legend>
      <Toggle
        label="Hash project names"
        checked={redaction.hashProjectNames}
        onChange={(hashProjectNames) => {
          onChange({ ...redaction, hashProjectNames });
        }}
      />
      <Toggle
        label="Leave out folder paths"
        checked={pathsLeftOut(redaction)}
        disabled={redaction.hashProjectNames}
        onChange={(dropPaths) => {
          onChange({ ...redaction, dropPaths });
        }}
      />
    </fieldset>
  );
}

/** Saves the report, then says how many rows were written. */
function SaveActions({ request, onClose }: { request: ExportRequest; onClose: () => void }) {
  const save = useCommandAction("save_export");
  return (
    <>
      <FormError error={save.error} />
      {save.data?.saved === true ? (
        <p role="status" className="text-xs text-success" data-numeric>
          Saved {String(save.data.rowCount)} rows.
        </p>
      ) : null}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onClose}>
          Close
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={save.isPending}
          onClick={() => {
            save.mutate(request);
          }}
        >
          {save.isPending ? "Saving…" : "Save…"}
        </Button>
      </div>
    </>
  );
}

export function ExportDialog({
  initialPreset,
  onClose,
}: {
  initialPreset: ExportPreset;
  onClose: () => void;
}) {
  const { scope } = useFilters();
  const [preset, setPreset] = useState(initialPreset);
  const [format, setFormat] = useState<ExportFormat>("csv");
  const [redaction, setRedaction] = useState<ExportRedaction>({
    hashProjectNames: false,
    dropPaths: false,
  });
  const request: ExportRequest = { scope, preset, format, redaction };
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title="Export report"
      showTitle
    >
      <div className="grid max-h-[70vh] gap-4 overflow-y-auto p-5">
        <p className="text-xs text-muted">
          The report covers {scope.start} to {scope.end} with the current filters, and adds up to
          the figures on the pages.
        </p>
        <div className="flex flex-wrap items-end gap-3">
          <div className="min-w-48 flex-1">
            <SelectField label="Report" value={preset} options={PRESETS} onChange={setPreset} />
          </div>
          <Segmented label="Format" value={format} options={FORMATS} onChange={setFormat} />
        </div>
        {namesProjects(preset) ? <Privacy redaction={redaction} onChange={setRedaction} /> : null}
        <Preview request={request} />
        <SaveActions request={request} onClose={onClose} />
      </div>
    </Modal>
  );
}
