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

fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, String> {
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
        body: InvokeBody::Json(json!({ "request": {} })),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    };
    get_ipc_response(window, request)
        .map(|body| body.deserialize::<Value>().expect("JSON response"))
        .map_err(|error| error.to_string())
}

#[test]
fn main_window_can_call_declared_command() {
    let app = app();
    let response = invoke(&window(&app, "main"), "get_app_info").expect("command is allowed");

    assert_eq!(response["productName"], "Veyra");
    assert_eq!(response["os"], std::env::consts::OS);
}

#[test]
fn other_windows_are_denied() {
    let app = app();
    let error = invoke(&window(&app, "other"), "get_app_info").expect_err("must be denied");

    assert!(error.contains("not allowed"), "unexpected error: {error}");
}

#[test]
fn undeclared_commands_are_rejected() {
    let app = app();
    let error = invoke(&window(&app, "main"), "delete_everything").expect_err("must be rejected");

    assert!(error.contains("not found"), "unexpected error: {error}");
}
