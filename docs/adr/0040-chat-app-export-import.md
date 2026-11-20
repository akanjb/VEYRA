# 0040. Importing chat-app data exports

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

ChatGPT, Claude.ai and the Gemini app keep their usage on the provider's servers and report no tokens to the user. Reading their cookies or calling private endpoints is excluded (ADR-014). Each provider does offer an official data export that lists every message. The coverage assessment ([06-universal-coverage.md](../planning/06-universal-coverage.md), §3.2) names this export as the only route to that usage.

Constraints:
- No text of any conversation may be stored or logged (NFR-03).
- Exports can be hundreds of megabytes; memory must stay bounded.
- Ingestion must not learn any export format (formats live in connectors).

## Decision

1. **The user picks the file.**
   - A card on the Sources page opens the system file dialog (`import_export`).
   - It accepts the `.zip` archive as the provider sends it, or the extracted `conversations.json`.
   - Nothing is read in the background, and Veyra keeps no path to the file.
2. **Connectors own the formats.**
   - The domain defines `ExportImporter`, one per app in `veyra-connectors::exports`, and `ArchiveImport`, the ingest service.
   - Each importer returns `Ok(None)` for a file that is not its export, so the first importer that recognises a file reads it.
   - The archive is opened in place. Its top-level JSON array is streamed one conversation at a time as a raw value, so memory is bounded by the largest conversation, not by the export.
3. **ChatGPT first.** Every assistant message becomes one record:
   - output: the message's own text; reasoning summaries count as reasoning;
   - input: the text of every message above it in the conversation tree, capped at 128,000 tokens;
   - model: the message's `model_slug`, else the conversation's default;
   - natural key `chatgpt:{conversation}:{message}`, session `chatgpt:{conversation}`, interface Browser.

   Text is read only to be measured and is dropped with its conversation.
4. **Estimated tokens.**
   - Counts use a fixed ratio: four ASCII characters, or one and a half other characters, per token.
   - Records are marked Estimated.
   - *Deviation from the plan:* the assessment proposed bundling OpenAI's tokenizer for ChatGPT. That was not done, for three reasons:
     - its vocabulary adds several megabytes to the installer;
     - it differs between model generations;
     - the input side is approximate anyway, because ChatGPT trims and rewrites context it does not export.

     The ratio stays within the error that remains either way.
5. **Same pipeline.**
   - Records are priced and stored through `push::store_usage` in batches of 2,000, like the receiver's and the proxy's.
   - Importing the same or a newer export again changes only what is new.
6. **Audit without personal data.** Each import is recorded with the app and its counts. The file name is never recorded.

## Consequences

- ChatGPT usage on every surface (web, desktop, mobile) becomes visible, with API-equivalent cost, without network access or credentials.
- Estimates err in both directions, and the Estimated badge says so:
  - images, files and tool output count nothing, so multimodal chats are underestimated;
  - trimmed context is still counted, so long chats are overestimated.
- Usage appears only as recently as the last export the user imports.
- A damaged conversation is skipped and counted. A damaged archive stops the import; records already stored are kept.
- Claude.ai and Gemini app exports can be added as further importers with no change to ingestion.
- The deletion command accepts imported apps. The Storage page lists only file sources, so its picker cannot choose them yet.
