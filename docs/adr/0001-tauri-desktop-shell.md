# 0001. Tauri 2 as the desktop shell

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

Veyra must run locally on Windows, macOS and Linux with low memory and CPU, a small installer, a strong security model for the webview, and no paid infrastructure. The dashboard is a web UI. The core does file parsing and database work.

## Decision

Use **Tauri 2** (pinned `tauri = 2.12.5`, CLI 2.12.4) with a Rust core and the system webview (WebView2 / WKWebView / WebKitGTK 4.1).

## Alternatives considered

- **Electron 45:** a single Chromium on every OS, but much larger installers and baseline RAM. Kept as a fallback only if webview inconsistencies become unmanageable.
- **Wails v3:** Go backend. The Rust ecosystem fits better for SQLite, filesystem watching and the OS keyring, and Tauri's capability ACL and updater are more mature.
- **Electrobun, Neutralinojs:** too young, or no comparable security and updater story.
- **Tauri 3:** alpha.

## Consequences

- Rendering differs between three webview engines, so it must be tested on each (CI builds on all three).
- Security advisories in Tauri 2.x are fixed at or above 2.11.6. We stay current via the supply-chain policy (ADR-017).

## Revisit when

Tauri 3 reaches stable, or WebKitGTK/WKWebView defects block product features.
