# 0020. IPC and webview security baseline

- **Status:** Accepted
- **Date:** 2026-11-08

## Context

The webview renders strings derived from local files (project names, model ids), so we must assume it could be exposed to XSS. A compromised page must not gain privileged access.

## Decision

- **Command allow-list.** `tauri_build::AppManifest::commands(COMMAND_NAMES)` turns every app command into a permission (`allow-<name>`). `capabilities/main.json` grants them to the `main` window only. No core plugin permissions are granted.
- **No dangerous plugins.** The `shell` and `sql` plugins are not used. `withGlobalTauri` is off, and the prototype is frozen.
- **Production CSP:**
  - `default-src 'self'` and `script-src 'self'`; Tauri adds only a hash for its own IPC bootstrap script;
  - `style-src 'self'`;
  - `connect-src ipc: http://ipc.localhost`;
  - `object-src`, `base-uri`, `form-action` and `frame-ancestors` set to `'none'`.
- **Development CSP.** The development CSP additionally allows inline scripts and styles plus the Vite websocket.
- **Dev server binding.** The dev server binds to `127.0.0.1` explicitly, not to the platform's resolution of `localhost`.

## Verification (M0)

- **Automated tests.** `src-tauri/tests/ipc_access.rs` checks that:
  - the `main` window can call `get_app_info`;
  - other windows get "not allowed";
  - unknown commands are rejected.
- **Manual check (2026-11-08).** Inspected through WebView2 DevTools on a production-asset build:
  - the served CSP header matched the configuration;
  - an injected `<style>` element and a `data:` script were blocked.

## Alternatives considered

- **Tauri defaults (all app commands callable from every window):** broader than needed.

## Consequences

- Each new command needs an `allow-*` entry in the capability. The tests fail otherwise, which is intentional.

## Revisit when

Multiple windows or remote content are introduced.
