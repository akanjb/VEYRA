import type { Capability, SupportLevel } from "@veyra/contracts";
import { Check, CircleDashed, Minus } from "lucide-react";

import { cn } from "@/shared/ui";

/** Words and an icon for each level; colour is never the only signal. */
const LEVEL: Readonly<Record<SupportLevel, { word: string; icon: typeof Check; tone: string }>> = {
  full: { word: "Reported", icon: Check, tone: "text-success" },
  partial: { word: "Partly", icon: CircleDashed, tone: "text-warning" },
  unsupported: { word: "Not reported", icon: Minus, tone: "text-subtle" },
};

/** What a source can report, with the limitation of each partial item in plain words. */
export function CapabilityMatrix({ capabilities }: { capabilities: readonly Capability[] }) {
  if (capabilities.length === 0) return null;
  return (
    <dl className="mt-4 grid gap-x-6 gap-y-2 border-t border-border pt-4 text-sm sm:grid-cols-2 lg:grid-cols-3">
      {capabilities.map((capability) => {
        const level = LEVEL[capability.support];
        const Icon = level.icon;
        return (
          <div key={capability.label} className="min-w-0">
            <dt className="text-xs text-subtle">{capability.label}</dt>
            <dd className="flex items-center gap-1.5">
              <Icon size={14} className={cn("shrink-0", level.tone)} aria-hidden />
              {level.word}
            </dd>
            {capability.note === null ? null : (
              <dd className="text-xs text-muted">{capability.note}</dd>
            )}
          </div>
        );
      })}
    </dl>
  );
}
