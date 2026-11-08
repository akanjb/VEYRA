# 0018. Unsigned v1 distribution with checksums

- **Status:** Accepted (decision D5, approved 2026-11-08)
- **Date:** 2026-11-08

## Context

OS code signing costs money: Apple Developer Program for notarization, and OV certificates or Azure Artifact Signing for Windows. Veyra must have no recurring cost.

## Decision

- **Release artefacts.** Ship v1 installers unsigned. Every release includes signed SHA-256 checksums, CycloneDX SBOMs and build-provenance attestations.
- **Install documentation.** Document the Windows SmartScreen and macOS Gatekeeper steps.
- **Free signing.** Apply to the SignPath Foundation free open-source signing programme (possible because of ADR-019).
- **Updater key.** The Tauri updater signing key, which is separate from OS signing, is mandatory and is kept offline with a documented rotation process.

## Alternatives considered

- **Paid certificates:** recurring cost.
- **Self-signed certificates:** no user trust benefit.

## Consequences

- Some users will see OS warnings. Smart App Control may block unsigned apps on some Windows 11 machines.

## Revisit when

SignPath approves the project, or a sponsor covers signing costs.
