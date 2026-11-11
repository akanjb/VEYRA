/** Renaming and merging projects: small confirmed forms, with the core's errors inline. */
import type { ProjectInfo } from "@veyra/contracts";
import { useId, useState } from "react";

import { useCommandMutation } from "@/shared/api";
import { Button, Modal } from "@/shared/ui";

import { MAX_NAME_CHARS, mergeTargets } from "./projects-model";

function ErrorLine({ error }: { error: Error | null }) {
  if (error === null) return null;
  return (
    <p role="alert" className="text-xs text-danger">
      {error.message}
    </p>
  );
}

function Actions({
  busy,
  submit,
  onCancel,
}: {
  busy: boolean;
  submit: string;
  onCancel: () => void;
}) {
  return (
    <div className="flex justify-end gap-2">
      <Button type="button" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
      <Button type="submit" variant="primary" disabled={busy}>
        {submit}
      </Button>
    </div>
  );
}

export function RenameDialog({ project, onClose }: { project: ProjectInfo; onClose: () => void }) {
  const rename = useCommandMutation("rename_project");
  const [name, setName] = useState(project.name ?? "");
  const field = useId();
  const submit = (event: { preventDefault: () => void }) => {
    event.preventDefault();
    rename.mutate(
      { id: project.id, name: name.trim() === "" ? null : name },
      { onSuccess: onClose },
    );
  };
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title="Rename project"
      showTitle
    >
      <form onSubmit={submit} className="grid gap-3 p-5">
        <p className="text-xs break-all text-muted">{project.path}</p>
        <label htmlFor={field} className="grid gap-1 text-sm">
          Name
          <input
            id={field}
            value={name}
            maxLength={MAX_NAME_CHARS}
            placeholder="Leave empty to show the folder path"
            onChange={(e) => {
              setName(e.target.value);
            }}
            className="h-8 rounded-control border border-border bg-surface px-2"
          />
        </label>
        <ErrorLine error={rename.error} />
        <Actions busy={rename.isPending} submit="Save" onCancel={onClose} />
      </form>
    </Modal>
  );
}

export function MergeDialog({
  project,
  projects,
  onClose,
}: {
  project: ProjectInfo;
  projects: readonly ProjectInfo[];
  onClose: () => void;
}) {
  const merge = useCommandMutation("merge_project");
  const targets = mergeTargets(project, projects);
  const [into, setInto] = useState<number | null>(targets[0]?.id ?? null);
  const field = useId();
  const target = targets.find((t) => t.id === into);
  const submit = (event: { preventDefault: () => void }) => {
    event.preventDefault();
    if (into !== null) merge.mutate({ from: project.id, into }, { onSuccess: onClose });
  };
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title="Merge project"
      showTitle
    >
      <form onSubmit={submit} className="grid gap-3 p-5">
        <label htmlFor={field} className="grid gap-1 text-sm">
          Merge {project.label} into
          <select
            id={field}
            value={into ?? ""}
            onChange={(e) => {
              setInto(Number(e.target.value));
            }}
            className="h-8 rounded-control border border-border bg-surface px-2"
          >
            {targets.map((t) => (
              <option key={t.id} value={t.id}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <p className="text-xs text-muted">
          All usage of {project.label}, past and future, will count toward{" "}
          {target?.label ?? "the chosen project"}. Nothing is deleted, and the merge can be undone
          from that project's page.
        </p>
        <ErrorLine error={merge.error} />
        <Actions busy={merge.isPending || into === null} submit="Merge" onCancel={onClose} />
      </form>
    </Modal>
  );
}
