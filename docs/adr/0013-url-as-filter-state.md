# 0013. TanStack Router; the URL is the single source of filter state

- **Status:** Accepted (adopted in M3)
- **Date:** 2026-11-08

## Context

Global filters (period, comparison, provider, model, project, …) must apply consistently across pages, survive back/forward navigation and deep links, and work later over localhost and Hub transports.

## Decision

- **Filter state lives in the URL.** Global filter state is stored in URL search params, validated by TanStack Router's `validateSearch` through one filter codec.
- **Server data.** Server data lives in TanStack Query.
- **No global store.** There is no global client store (no Zustand) until a concrete need appears.

## Alternatives considered

- **React Router 8:** less typed search-param support.
- **Zustand plus URL syncing:** two sources of truth for the same state.

## Consequences

- Exact version pins and the release-age gate (ADR-017) mitigate the May 2026 `@tanstack/*` compromise (GHSA-g7cv-rxg3-hmpx).

## Revisit when

Router maintenance or security posture degrades.
