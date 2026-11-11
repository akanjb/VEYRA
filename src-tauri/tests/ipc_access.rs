//! IPC access-control tests: only the `main` window may call application commands,
//! and only commands declared in the catalogue exist.

#![expect(
    clippy::expect_used,
    reason = "test setup failures should abort the test"
)]

use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

fn app() -> App<MockRuntime> {
    let context = tauri::generate_context!();
    veyra_desktop::configure(mock_builder(), &context)
        .build(context)
        .expect("app builds")
}

/// The mock runtime does not create the windows declared in `tauri.conf.json`.
fn window(app: &App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::default())
        .build()
        .expect("window builds")
}

/// Invokes an application command with a well-formed request.
fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, String> {
    invoke_with(window, cmd, json!({ "request": valid_request(cmd) }))
}

fn invoke_with(
    window: &WebviewWindow<MockRuntime>,
    cmd: &str,
    body: Value,
) -> Result<Value, String> {
    let origin = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: origin.parse().expect("valid origin"),
        body: InvokeBody::Json(body),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    };
    get_ipc_response(window, request)
        .map(|body| body.deserialize::<Value>().expect("JSON response"))
        .map_err(|error| error.to_string())
}

/// A well-formed request body for each command.
fn valid_request(cmd: &str) -> Value {
    let scope = json!({
        "timeZone": "UTC",
        "start": "2026-11-01",
        "end": "2026-11-08",
        "comparison": "previous",
        "filters": { "providers": [], "models": [], "apps": [], "interfaces": [], "projects": [] }
    });
    match cmd {
        "set_source_enabled" => json!({ "sourceId": 1, "enabled": false }),
        "get_overview" | "get_usage" | "get_cache" | "get_models" => json!({ "scope": scope }),
        "list_sessions" => {
            json!({ "scope": scope, "sort": "lastActive", "page": 0, "pageSize": 50 })
        }
        "get_session" => json!({ "id": 1 }),
        _ => json!({}),
    }
}

#[test]
fn main_window_can_call_declared_command() {
    let app = app();
    let response = invoke(&window(&app, "main"), "get_app_info").expect("command is allowed");

    assert_eq!(response["productName"], "Veyra");
    assert_eq!(response["os"], std::env::consts::OS);
}

#[test]
fn main_window_reaches_every_catalogue_command() {
    let app = app();
    let main = window(&app, "main");
    for command in veyra_app::COMMAND_NAMES {
        if let Err(error) = invoke(&main, command) {
            // Data services are not started under test: the command ran and said so.
            assert!(
                error.contains("\"busy\""),
                "{command}: unexpected error: {error}"
            );
        }
    }
}

#[test]
fn other_windows_are_denied_every_command() {
    let app = app();
    let other = window(&app, "other");
    for command in veyra_app::COMMAND_NAMES {
        let error = invoke(&other, command).expect_err("must be denied");
        assert!(
            error.contains("not allowed"),
            "{command}: unexpected error: {error}"
        );
    }
}

/// What `listen()` of `@tauri-apps/api/event` sends.
fn listen_request() -> Value {
    json!({
        "event": veyra_app::events::DATA_CHANGED,
        "target": { "kind": "Any" },
        "handler": 7,
    })
}

#[test]
fn main_window_may_listen_to_application_events() {
    let app = app();
    let main = window(&app, "main");
    let id = invoke_with(&main, "plugin:event|listen", listen_request()).expect("may listen");
    let unlisten = json!({ "event": veyra_app::events::DATA_CHANGED, "eventId": id });
    invoke_with(&main, "plugin:event|unlisten", unlisten).expect("may unlisten");
}

#[test]
fn other_windows_may_not_listen() {
    let app = app();
    let error = invoke_with(
        &window(&app, "other"),
        "plugin:event|listen",
        listen_request(),
    )
    .expect_err("must be denied");
    assert!(error.contains("not allowed"), "unexpected error: {error}");
}

#[test]
fn main_window_may_not_emit_or_use_other_plugins() {
    let app = app();
    let main = window(&app, "main");
    let emit = json!({ "event": veyra_app::events::DATA_CHANGED, "payload": null });
    for (cmd, body) in [
        ("plugin:event|emit", emit),
        ("plugin:window|close", json!({ "label": "main" })),
    ] {
        let error = invoke_with(&main, cmd, body).expect_err("must be denied");
        assert!(
            error.contains("not allowed"),
            "{cmd}: unexpected error: {error}"
        );
    }
}

#[test]
fn undeclared_commands_are_rejected() {
    let app = app();
    let error = invoke(&window(&app, "main"), "delete_everything").expect_err("must be rejected");

    assert!(error.contains("not found"), "unexpected error: {error}");
}
