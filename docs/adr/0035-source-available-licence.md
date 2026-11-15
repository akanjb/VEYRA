# 0035. Source-available licence

- **Status:** Accepted, supersedes [ADR-019](0019-licence.md)
- **Date:** 2026-11-15

## Context

The repository is becoming public so that anyone can use Veyra. The author wants to keep sole ownership and control:
- no one may sell the code, rebrand it or publish a modified copy or clone;
- no one becomes a co-owner or gains write access by contributing;
- the author carries no liability for how the software or its figures are used.

Apache-2.0 OR MIT (ADR-019) permits all of the first point, so it no longer fits.

## Decision

1. **Licence.** Veyra is published under the **Veyra Source-Available License 1.0** (`LICENSE`). Cargo records it as `LicenseRef-Veyra-Source-Available-1.0`. It allows anyone to:
   - read the code;
   - build and run unmodified Veyra for personal use or an organisation's internal use;
   - modify a copy only to prepare a contribution;
   - fork only as far as GitHub's terms require for public repositories.

   Selling, redistribution, derivative works, clones, hosted services, removing notices, claiming authorship, using the name or branding, and training AI models on the code are all forbidden without written permission.
2. **Contributions.** A contribution assigns its rights to the author, with a broad irrevocable licence as a fallback where an assignment is not effective. Acceptance and write access stay at the author's sole discretion. Contributing gives no ownership or credit rights.
3. **Liability.** The licence disclaims all warranties and limits liability. It states that every figure is an estimate and not financial advice, and that users are responsible for their own data. Users indemnify the author.
4. **Third-party code.** Dependencies keep their own licences. cargo-deny still enforces the allow-list for them and skips Veyra's own unpublished crates.

## Alternatives considered

- **Keeping Apache-2.0 OR MIT.** Anyone could sell or rebrand the code.
- **PolyForm Strict or Noncommercial.** These are well-known texts, but they forbid use at work, or they permit modified redistribution.
- **Business Source License.** It converts to open source after a delay, which the author does not want.
- **No licence (all rights reserved).** It is the strongest default, but users would get no clear right to run the software, and there would be no contribution terms or liability disclaimer.

## Consequences

- Veyra is not open source under the OSI definition. It no longer qualifies for SignPath Foundation's free code signing, which needs an OSI licence (ADR-018 already ships v1 unsigned).
- Package managers and directories that accept only OSI licences will not list it.
- Enforcement relies on copyright, for example DMCA takedown notices to GitHub.

## Revisit when

The author wants outside redistribution, a commercial licence, or open-source code signing. Have a lawyer review the licence before relying on it in a dispute.
