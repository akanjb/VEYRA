# ChatGPT data export

Veyra can import the usage recorded in ChatGPT's official data export (ADR-040). The import covers ChatGPT on the web, desktop and mobile, whose usage OpenAI otherwise keeps on its servers.

## Importing

1. In ChatGPT, open Settings, then Data controls, then Export data, and confirm. OpenAI emails a download link.
2. Download the `.zip` archive.
3. In Veyra, open Sources, go to the **Chat app exports** card, and choose **Import data export…**.
4. Pick the archive, or the `conversations.json` file taken from it.

Importing a newer export later adds only the new replies.

## What is recorded

Veyra records one usage record per assistant reply, marked **Estimated**:

| Field | Source |
|---|---|
| Time | The reply's creation time, else the conversation's |
| Model | The reply's `model_slug`, else the conversation's default model |
| Output tokens | The reply's text; reasoning summaries count as reasoning tokens |
| Input tokens | The text of every message above the reply, capped at 128,000 tokens |
| Session | The conversation id |

Tokens are estimated at four ASCII characters, or one and a half other characters, per token.

## What is not recorded

- The text of any message. Text is read only to be measured.
- Conversation titles and attachments.
- The export file's name and location.
- Images and uploaded files. They count no tokens, so multimodal conversations are underestimated.

## Limits

- ChatGPT trims long conversations before sending them to the model. The input of very long chats can therefore be overestimated.
- Cost is the API-equivalent cost of the same tokens, not what a ChatGPT plan charges.
- Conversations whose shape Veyra does not understand are skipped and counted in the result.
