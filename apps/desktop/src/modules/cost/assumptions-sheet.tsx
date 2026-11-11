/** The cost assumptions panel: how the scope's API-equivalent cost was estimated. */
import type {
  AssumptionUse,
  CostAssumptions,
  PricingTally,
  RuleUse,
  Scope,
  StatusUse,
  UnpricedModel,
} from "@veyra/contracts";
import { useState } from "react";
import type { ReactNode } from "react";

import { useCommand } from "@/shared/api";
import { MEASURE, fmt } from "@/shared/format";
import { Button, ErrorState, Sheet, Skeleton } from "@/shared/ui";

import {
  STATUS_LABEL,
  coveredModels,
  effectiveText,
  modifierText,
  rateRows,
  summaryText,
} from "./assumptions-model";

/** "12 requests · ≈$4.10". */
function TallyText({ tally }: { tally: PricingTally }) {
  return (
    <span className="text-xs text-muted" data-numeric>
      {fmt.integer(tally.events)} {tally.events === 1 ? "request" : "requests"} ·{" "}
      {MEASURE.cost.format(tally.apiEquivalentNanoUsd)}
    </span>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="grid gap-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}

function List({ children }: { children: ReactNode }) {
  return <ul className="grid gap-2">{children}</ul>;
}

function StatusItem({ use }: { use: StatusUse }) {
  return (
    <li className="flex flex-wrap items-baseline justify-between gap-x-3">
      <span className="text-sm">{STATUS_LABEL[use.status]}</span>
      <TallyText tally={use.tally} />
    </li>
  );
}

function AssumptionItem({ use }: { use: AssumptionUse }) {
  return (
    <li className="grid gap-0.5 rounded-control border border-border p-2">
      <span className="text-sm break-words">{use.description}</span>
      <span className="flex flex-wrap gap-x-2">
        <TallyText tally={use.tally} />
        {use.makesPartial ? <span className="text-xs text-warning">Cost not exact</span> : null}
      </span>
    </li>
  );
}

function RuleItem({ rule }: { rule: RuleUse }) {
  const effective = effectiveText(rule);
  return (
    <li className="grid gap-1.5 rounded-control border border-border p-2">
      <div className="flex flex-wrap items-baseline gap-x-2">
        <span className="min-w-0 text-sm font-medium break-words">{coveredModels(rule)}</span>
        <span className="text-xs text-muted">{rule.provider.label}</span>
        {rule.isStale ? (
          <span className="rounded-full border border-warning px-1.5 text-xs text-warning">
            Stale
          </span>
        ) : null}
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
        {rateRows(rule.base).map((row) => (
          <div key={row.label} className="contents">
            <dt className="text-subtle">{row.label}</dt>
            <dd data-numeric>{row.value}</dd>
          </div>
        ))}
      </dl>
      {rule.tiers.map((tier) => (
        <p key={tier.abovePromptTokens} className="text-xs text-muted" data-numeric>
          Prompts over {fmt.integer(tier.abovePromptTokens)} tokens:{" "}
          {rateRows(tier.rates)
            .map((r) => `${r.label} ${r.value}`)
            .join(", ")}
        </p>
      ))}
      {rule.modifiers.length > 0 ? (
        <p className="text-xs text-muted">{rule.modifiers.map(modifierText).join(" · ")}</p>
      ) : null}
      <p className="text-xs text-muted">
        {effective === null ? "" : `${effective} · `}Checked {fmt.day(rule.verifiedAt)}
        {rule.historyAssumed ? " · older requests use the same price" : ""}
      </p>
      <p className="text-xs break-all text-subtle select-text">{rule.sourceUrl}</p>
      <TallyText tally={rule.tally} />
    </li>
  );
}

function UnpricedItem({ model }: { model: UnpricedModel }) {
  return (
    <li className="flex flex-wrap items-baseline justify-between gap-x-3">
      <span className="min-w-0 text-sm break-words">
        {model.model?.label ?? "Unnamed model"}{" "}
        <span className="text-xs text-muted">{model.provider.label}</span>
      </span>
      <span className="text-xs text-muted" data-numeric>
        {MEASURE.tokens.format(model.tally.tokens)} tokens
      </span>
    </li>
  );
}

function AssumptionsBody({ data }: { data: CostAssumptions }) {
  return (
    <div className="grid gap-5">
      <p className="text-sm text-muted">
        {summaryText(data)}. {data.definition}
      </p>
      {data.statuses.length > 0 ? (
        <Section title="Outcomes">
          <List>
            {data.statuses.map((use) => (
              <StatusItem key={use.status} use={use} />
            ))}
          </List>
        </Section>
      ) : null}
      {data.assumptions.length > 0 ? (
        <Section title="Assumptions">
          <List>
            {data.assumptions.map((use) => (
              <AssumptionItem key={use.kind} use={use} />
            ))}
          </List>
        </Section>
      ) : null}
      {data.rules.length > 0 ? (
        <Section title="Prices applied">
          <List>
            {data.rules.map((rule) => (
              <RuleItem key={rule.id} rule={rule} />
            ))}
          </List>
        </Section>
      ) : null}
      {data.unpriced.length > 0 ? (
        <Section title="Without a published price">
          <List>
            {data.unpriced.map((model) => (
              <UnpricedItem key={`${model.provider.key}/${model.model?.key ?? ""}`} model={model} />
            ))}
          </List>
        </Section>
      ) : null}
      <p className="text-xs text-subtle">Price catalogue {data.catalogVersion}</p>
    </div>
  );
}

function AssumptionsContent({ scope }: { scope: Scope }) {
  const query = useCommand("get_cost_assumptions", { scope });
  if (query.isPending) return <Skeleton className="h-40" />;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  return <AssumptionsBody data={query.data} />;
}

/** A button opening the panel; the panel loads only when opened. */
export function AssumptionsButton({ scope }: { scope: Scope }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        size="sm"
        onClick={() => {
          setOpen(true);
        }}
      >
        How cost is estimated
      </Button>
      <Sheet
        open={open}
        onOpenChange={setOpen}
        title="How cost is estimated"
        description="Prices and assumptions behind the API-equivalent cost"
      >
        {open ? <AssumptionsContent scope={scope} /> : null}
      </Sheet>
    </>
  );
}
