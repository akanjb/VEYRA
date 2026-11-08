# 0024. Dashboard frontend: stack, design system and deviations from the plan

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

M3 delivers the dashboard shell, the design system and three pages (Overview, Sessions with explorer and detail, Sources & Health with the first-run consent flow). The plan ([`docs/planning`](../planning)) named a frontend stack and a test approach. Building the pages raised questions the plan did not settle:
- which libraries are worth their weight;
- where display names and definitions come from;
- how the dashboard is developed and tested without the desktop shell;
- what the window needs from Tauri beyond the application commands.

## Decision

1. **Libraries (exact versions, supply-chain policy of ADR-017):**
   - TanStack Router for routes and URL state (ADR-013);
   - TanStack Query for command results, refreshed by the core's `data-changed` event;
   - Radix primitives (`radix-ui`) for menus, dialogs, tooltips, switches and toggle groups: accessible behaviour without a visual style;
   - Tailwind CSS 4 over design tokens;
   - Apache ECharts (ADR-011) and lucide icons;
   - Inter and JetBrains Mono, bundled locally (no font requests).

2. **Design tokens are the single source of visual truth.** `styles/tokens.css` defines every colour, radius, shadow and type size for the light and dark themes. Tailwind maps them with `@theme inline`, and charts read the same variables at runtime.
   - The ten categorical data colours are validated by `tooling/palette/check-palette.js`, which runs inside `pnpm test`: at least 3:1 contrast against the card surface, CIEDE2000 ΔE ≥ 10 between any two slots, and ΔE ≥ 5 between the first six slots under simulated protanopia, deuteranopia and tritanopia.
   - Providers keep identity colours (slots 1–3). A provider's models are tints of its hue; other categories start at slot 4.

3. **The core names and defines; the dashboard displays.**
   - KPI definitions come from the metric registry (`Kpi.definition`).
   - Providers, models and applications reach the dashboard as `Labelled { key, label }`, named by the registry and the connector descriptors through one `DisplayNames` value.
   - Unknown values stay unknown: an empty period yields `null`, shown as "—", never 0.

4. **Strict module boundaries**, enforced by the `veyra/module-boundaries` lint rule:
   - `shared/` (API client, filters, formatting, UI and chart primitives) imports neither pages nor the shell;
   - a page module (`modules/overview`, `modules/sessions`, `modules/sources`) imports no other page;
   - only `shared/api` may import `@tauri-apps/*`.

5. **A synthetic fixture client.** Outside Tauri, in development and UI tests only, a deterministic synthetic dataset implements the same `ApiClient` as the IPC client: same commands, errors, consent behaviour and `data-changed` events. `?fresh` starts like a first run. It is loaded by dynamic import and never shipped in a production build.

6. **UI tests with Playwright and axe**, against the fixture client, in CI on Ubuntu:
   - the first-run consent journey (UC-01);
   - the keyboard-only session investigation (UC-02);
   - URL filter state with Back and Forward;
   - axe WCAG 2.2 A/AA checks of every page in both themes.

7. **The window may listen to the application's events.** The `main` capability adds `core:event:allow-listen` and `core:event:allow-unlisten`, and nothing else from the core plugins. Emitting events and every other plugin stay denied; IPC tests assert both. This amends ADR-020.

## Alternatives considered

- **TanStack Table and TanStack Virtual.** Rejected for M3. Sessions are paged by the core (50 per page), so the explorer never holds enough rows to need virtualisation, and the plain accessible `DataTable` covers sorting by URL. Revisit with the Models and Projects pages in M5.
- **cmdk for the command palette.** Rejected. Filtering a few dozen actions by substring is a few lines; the palette is a Radix dialog with a WAI-ARIA combobox and listbox.
- **Names in the dashboard.** Rejected. A mapping of ids to names in TypeScript would duplicate the registry and drift from it.
- **A mocked IPC layer per test.** Rejected in favour of one fixture client that behaves like the core, so development, screenshots and UI tests exercise the same paths.
- **WebdriverIO against real bundles.** Deferred: the smoke test on built bundles for all three operating systems needs the release pipeline (M8).

## Consequences

- The interface filter is supported by the core and the URL, but has no control yet: Claude Code reports only one interface. It appears with the second connector (M4).
- Scatter and waterfall charts and the Overview highlights move to M5, where the Cost and Cache pages need them.
- A visual check on Linux (WebKitGTK) is deferred to M8; the UI tests run in Chromium.
- The moderated usability test (five participants) is a manual step before the internal alpha, not part of CI.
- Contract changes now touch three places: the DTO, the fixture client and the page. `pnpm contracts:check`, the type checker and the UI tests catch drift between them.

## Revisit when

- A page needs more than a few hundred client-side rows (virtualisation).
- The palette check rejects a needed new slot (palette redesign).
- A Tauri update changes what `core:event` permissions grant.
