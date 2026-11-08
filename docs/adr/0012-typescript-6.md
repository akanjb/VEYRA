# 0012. TypeScript 6.0 with type-aware ESLint

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

TypeScript 7.0.2 (the native port) was released on 2026-11-08. typescript-eslint 8.72.1, which provides the type-aware rules we rely on (`no-floating-promises`, `no-unsafe-*`, `strict-type-checked`), supports only TypeScript `<6.1.0`.

## Decision

- **Versions.** Use **TypeScript 6.0.3** in strict mode (plus `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes` and related flags) with ESLint 10 and typescript-eslint `strictTypeChecked`.
- **Formatting.** Prettier formats the code.

## Alternatives considered

- **TypeScript 7 now, with Biome or oxlint:** faster, but loses mature type-aware lint rules, and the release is brand new.

## Consequences

- A later migration to TypeScript 7.

## Revisit when

typescript-eslint officially supports TypeScript 7 and TS 7 has had at least one patch release.
