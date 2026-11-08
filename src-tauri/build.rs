use std::path::PathBuf;

/// Windows application manifest requiring Common Controls v6, identical to Tauri's
/// default. Tauri embeds it only into the app binary; test binaries need it too or
/// they abort with `STATUS_ENTRYPOINT_NOT_FOUND` when creating (mock) windows.
const COMMON_CONTROLS_V6_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;

fn main() {
    // Restrict IPC to the commands in the application catalogue. Each command gets an
    // `allow-<name>` permission that a capability must grant explicitly.
    let manifest = tauri_build::AppManifest::new().commands(veyra_app::COMMAND_NAMES);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
    embed_test_manifest_on_msvc();
}

fn embed_test_manifest_on_msvc() {
    let is_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let is_msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(is_windows && is_msvc) {
        return;
    }
    let Some(out_dir) = std::env::var_os("OUT_DIR").map(PathBuf::from) else {
        panic!("OUT_DIR is not set");
    };
    let path = out_dir.join("test-app-manifest.xml");
    if let Err(error) = std::fs::write(&path, COMMON_CONTROLS_V6_MANIFEST) {
        panic!("cannot write test manifest: {error}");
    }
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        path.display()
    );
}
