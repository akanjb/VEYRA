/** Pieces every small form shares: the core's error inline, and Cancel / submit. */
import type { ReactNode } from "react";

import { userMessage } from "../api";

import { Button } from "./primitives";

/** The error of the last attempt, announced to assistive technology. */
export function FormError({ error }: { error: Error | null }) {
  if (error === null) return null;
  return (
    <p role="alert" className="text-xs text-danger">
      {userMessage(error)}
    </p>
  );
}

export function FormActions({
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

/** A labelled form field; the label wraps its control. */
export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid gap-1 text-sm">
      {label}
      {children}
    </label>
  );
}

/** The class of text inputs and selects in forms. */
export const CONTROL = "h-8 rounded-control border border-border bg-surface px-2 text-sm text-text";

/** A labelled text input reporting its new value. */
export function TextField({
  label,
  value,
  onChange,
  ...input
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: "text" | "date" | "number";
  placeholder?: string;
  maxLength?: number;
  min?: number;
  max?: number;
  inputMode?: "decimal" | "text";
  "aria-invalid"?: boolean;
}) {
  return (
    <Field label={label}>
      <input
        className={CONTROL}
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
        }}
        {...input}
      />
    </Field>
  );
}

/** A labelled select of known values. */
export function SelectField<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <Field label={label}>
      <select
        className={CONTROL}
        value={value}
        onChange={(e) => {
          const option = options.find((o) => o.value === e.target.value);
          if (option !== undefined) onChange(option.value);
        }}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </Field>
  );
}
