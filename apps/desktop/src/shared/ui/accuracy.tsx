/**
 * Accuracy is always visible (docs/planning/03-ux.md §1). Shape and text carry the class;
 * colour never does on its own.
 */
import type { AccuracyClass, Composition } from "@veyra/contracts";

import { fmt } from "../format";
import { Tip, cn } from "./primitives";

export type AccuracyLabel = AccuracyClass | "derived";

const META: Record<AccuracyLabel, { label: string; description: string }> = {
  exact: { label: "Exact", description: "Recorded directly by the source." },
  providerReported: {
    label: "Reported",
    description: "Supplied by the provider's own usage interface.",
  },
  partial: { label: "Partial", description: "Some events or fields are missing." },
  estimated: {
    label: "≈ Estimated",
    description: "Calculated with a documented method; not a bill.",
  },
  unsupported: { label: "Unsupported", description: "No reliable, permitted source exists." },
  derived: { label: "Derived", description: "Computed from exact timestamps with a stated rule." },
};

function Mark({ kind }: { kind: AccuracyLabel }) {
  const common = { width: 10, height: 10, viewBox: "0 0 10 10", "aria-hidden": true } as const;
  switch (kind) {
    case "exact":
      return (
        <svg {...common}>
          <circle cx="5" cy="5" r="4" fill="currentColor" />
        </svg>
      );
    case "providerReported":
      return (
        <svg {...common}>
          <circle cx="5" cy="5" r="2.5" fill="currentColor" />
          <circle cx="5" cy="5" r="4.2" fill="none" stroke="currentColor" strokeWidth="1" />
        </svg>
      );
    case "estimated":
      return (
        <svg {...common}>
          <circle
            cx="5"
            cy="5"
            r="4"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeDasharray="1.6 1.4"
          />
        </svg>
      );
    case "partial":
      return (
        <svg {...common}>
          <circle cx="5" cy="5" r="4" fill="none" stroke="currentColor" strokeWidth="1" />
          <path d="M5 1 A4 4 0 0 0 5 9 Z" fill="currentColor" />
        </svg>
      );
    case "unsupported":
      return (
        <svg {...common}>
          <path d="M1.5 5 H8.5" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      );
    case "derived":
      return (
        <svg {...common}>
          <path d="M5 1.2 L9 8.8 H1 Z" fill="none" stroke="currentColor" strokeWidth="1.1" />
        </svg>
      );
  }
}

export function AccuracyBadge({
  kind,
  className,
}: {
  kind: AccuracyLabel | null;
  className?: string;
}) {
  if (kind === null) return null;
  const meta = META[kind];
  return (
    <Tip content={meta.description}>
      <span
        className={cn(
          "inline-flex items-center gap-1 rounded-full border border-border px-1.5 py-px text-[11px] text-muted",
          className,
        )}
        tabIndex={0}
        aria-label={`Accuracy: ${meta.label}. ${meta.description}`}
      >
        <Mark kind={kind} />
        {meta.label}
      </span>
    </Tip>
  );
}

const PARTS: { key: keyof Omit<Composition, "weakest" | "pricingCoverage">; label: string }[] = [
  { key: "exact", label: "exact" },
  { key: "providerReported", label: "reported" },
  { key: "partial", label: "partial" },
  { key: "estimated", label: "estimated" },
  { key: "unsupported", label: "unsupported" },
];

/** Accuracy composition of the scope, e.g. "Exact 96% · Est 4%", and pricing coverage. */
export function CoverageMeter({ composition }: { composition: Composition | undefined }) {
  if (composition === undefined || composition.weakest === null) {
    return <span className="text-xs text-subtle">No data in this period</span>;
  }
  const parts = PARTS.filter((p) => composition[p.key] > 0);
  const text = parts.map((p) => `${fmt.percent(composition[p.key])} ${p.label}`).join(" · ");
  const pricing =
    composition.pricingCoverage === null
      ? null
      : `Priced: ${fmt.percent(composition.pricingCoverage)} of tokens`;
  return (
    <Tip
      content={
        <span>
          Token accuracy in this view: {text}.{pricing === null ? "" : ` ${pricing}.`}
        </span>
      }
    >
      <span tabIndex={0} className="inline-flex items-center gap-2 text-xs text-muted">
        <AccuracyBadge kind={composition.weakest} />
        <span data-numeric>{text}</span>
      </span>
    </Tip>
  );
}
