# 0019. Licence: Apache-2.0 OR MIT

- **Status:** Superseded by [ADR-035](0035-source-available-licence.md) (originally decision D2, approved 2026-11-08)
- **Date:** 2026-11-08

## Context

Veyra is free to use and should be easy to contribute to and redistribute. Free code signing through SignPath Foundation requires an OSI-approved licence with no commercial dual-licensing.

## Decision

- **Licence.** Veyra is licensed **Apache-2.0 OR MIT**, at the user's option (`LICENSE-APACHE`, `LICENSE-MIT`).
- **Contributions.** Contributions are accepted under the same terms (inbound = outbound).

## Alternatives considered

- **MIT only:** no explicit patent grant.
- **Apache-2.0 only:** less flexible for GPLv2 projects.
- **AGPL:** discourages adoption and organisation use.

## Consequences

- Matches the Rust ecosystem norm.
- Dependency licences must stay compatible; cargo-deny enforces the allow-list.

## Revisit when

Never expected.
