/** Importing the data export of a chat app such as ChatGPT (ADR-040). */
import type { ExportImported } from "@veyra/contracts";

import { useCommandMutation } from "@/shared/api";
import { fmt } from "@/shared/format";
import { Button, Card, FormError } from "@/shared/ui";

function resultText(result: ExportImported): string {
  if (!result.imported) return "No file was chosen.";
  const skipped =
    result.skipped === 0 ? "" : ` ${fmt.integer(result.skipped)} conversations could not be read.`;
  return `${result.app ?? "Export"}: ${fmt.integer(result.replies)} replies in ${fmt.integer(result.conversations)} conversations; ${fmt.integer(result.added)} added, ${fmt.integer(result.updated)} updated, ${fmt.integer(result.unchanged)} already imported.${skipped}`;
}

export function ImportCard() {
  const run = useCommandMutation("import_export");
  return (
    <Card title="Chat app exports">
      <div className="grid gap-3">
        <p className="text-sm text-muted">
          ChatGPT keeps its usage on OpenAI&apos;s servers. Request your data export in ChatGPT
          (Settings, Data controls, Export data), then choose the downloaded .zip here. Veyra keeps
          only times, models and token estimates; the text of your conversations is read once and
          never stored.
        </p>
        <ul className="grid list-disc gap-1 pl-4 text-xs text-muted">
          <li>Tokens are estimated from the text, so these records are marked Estimated.</li>
          <li>Importing the same or a newer export again adds only what is new.</li>
        </ul>
        <div>
          <Button
            size="sm"
            disabled={run.isPending}
            onClick={() => {
              run.mutate({});
            }}
          >
            {run.isPending ? "Importing…" : "Import data export…"}
          </Button>
        </div>
        {run.data === undefined ? null : (
          <p role="status" className="text-xs text-muted" data-numeric>
            {resultText(run.data)}
          </p>
        )}
        <FormError error={run.error} />
      </div>
    </Card>
  );
}
