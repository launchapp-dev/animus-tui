//! Library-mode entrypoint for `animus-tui` so smoke tests can drive the
//! `App` against an in-memory fake control client. The shipped binary lives
//! in `src/main.rs`.

pub mod app;
pub mod control_client;
pub mod keybinds;
pub mod theme;
pub mod views;
pub mod widgets;

pub use app::App;

/// JSON payload emitted by `animus-tui --manifest`. Kept in the library
/// so smoke tests can assert it parses without spawning the binary.
pub const PLUGIN_MANIFEST_JSON: &str = r#"{
  "name": "animus-tui",
  "version": "0.1.0",
  "plugin_kind": "custom",
  "description": "Terminal control plane for the Animus daemon — k9s-style UI for workflows, queue, subjects, logs, and cost.",
  "protocol_version": "0.1.14",
  "capabilities": [],
  "env_required": []
}
"#;
