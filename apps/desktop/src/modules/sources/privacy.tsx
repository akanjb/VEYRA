import { Check, X } from "lucide-react";

const READS = [
  "Timestamps, model names and token counts",
  "Session and request identifiers",
  "The working directory (to attribute usage to a project)",
  "Tool names and whether they failed",
];

const NEVER = [
  "Prompts, responses or conversation text",
  "Source code or file contents",
  "Tool inputs or outputs",
  "Credentials, keychains or cookies of any application",
];

/** The privacy promise in plain language (docs/planning/03-ux.md §7, first run). */
export function PrivacyPromise() {
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <div>
        <h3 className="mb-2 text-xs font-medium tracking-wide text-subtle uppercase">
          Veyra reads
        </h3>
        <ul className="grid gap-1.5 text-sm">
          {READS.map((item) => (
            <li key={item} className="flex gap-2">
              <Check size={16} className="mt-0.5 shrink-0 text-success" aria-hidden />
              {item}
            </li>
          ))}
        </ul>
      </div>
      <div>
        <h3 className="mb-2 text-xs font-medium tracking-wide text-subtle uppercase">
          Veyra never reads or stores
        </h3>
        <ul className="grid gap-1.5 text-sm">
          {NEVER.map((item) => (
            <li key={item} className="flex gap-2">
              <X size={16} className="mt-0.5 shrink-0 text-danger" aria-hidden />
              {item}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
