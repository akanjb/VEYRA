# Security Policy

Veyra reads local files that may contain sensitive information. We take reports seriously.

## Reporting a vulnerability

Please **do not** open a public issue. Report privately through GitHub's **"Report a vulnerability"** (Security Advisories) on this repository.

Include affected versions, reproduction steps and impact. We aim to acknowledge reports within 3 business days and agree a disclosure timeline with you, normally at most 90 days.

## Supported versions

Veyra is pre-release. Only the latest commit on `main` and, once published, the latest release receive fixes.

## Scope and design commitments

**In scope:** the desktop app, its Rust core, the dashboard, the build and release pipeline, and (later) Veyra Hub.

**By design, Veyra:**
- reads usage sources read-only and never modifies them;
- never reads other applications' credentials, keychains or cookie stores (ADR-014);
- stores no prompts, responses, tool arguments or file contents;
- sends no telemetry. Network access happens only for features you explicitly enable;
- restricts IPC to declared commands for the main window, under a strict CSP (ADR-020).

A violation of any of these is a security bug.

## Supply chain

Dependencies are pinned exactly, gated by release age, and scanned in CI (ADR-017). Release artefacts will carry SHA-256 checksums, SBOMs and build-provenance attestations (ADR-018).
