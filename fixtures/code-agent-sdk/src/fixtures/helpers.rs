//! Shared helpers for multi-backend fixture tests.
//!
//! Provides backend detection, environment-based CLI discovery, and common
//! assertion utilities used across all fixtures.

use std::path::Path;

use code_agent_sdk::{AgentOptions, BackendKind, CodexOptions, CursorOptions};

/// Information about an available backend.
#[derive(Debug, Clone)]
pub struct AvailableBackend {
    pub kind: BackendKind,
    pub cli_path: String,
}

/// Detect which CLI backends are available on the system.
///
/// Checks environment variables and PATH for each CLI binary:
/// - Claude: `CLAUDE_CLI_PATH` or `claude` on PATH
/// - Codex: `CODEX_CLI_PATH` or `codex` on PATH
/// - Cursor: `CURSOR_CLI_PATH` or `agent` on PATH
pub fn detect_available_backends() -> Vec<AvailableBackend> {
    let mut backends = Vec::new();

    if let Some(path) = find_cli_path("CLAUDE_CLI_PATH", "claude") {
        backends.push(AvailableBackend {
            kind: BackendKind::Claude,
            cli_path: path,
        });
    }

    if let Some(path) = find_cli_path("CODEX_CLI_PATH", "codex") {
        backends.push(AvailableBackend {
            kind: BackendKind::Codex,
            cli_path: path,
        });
    }

    if let Some(path) = find_cli_path("CURSOR_CLI_PATH", "agent") {
        backends.push(AvailableBackend {
            kind: BackendKind::Cursor,
            cli_path: path,
        });
    }

    backends
}

/// Check if a specific backend is available.
pub fn is_backend_available(kind: BackendKind) -> Option<AvailableBackend> {
    detect_available_backends()
        .into_iter()
        .find(|b| b.kind == kind)
}

/// Build `AgentOptions` for a specific backend with its CLI path set.
pub fn options_for_backend(backend: &AvailableBackend) -> AgentOptions {
    let mut builder = AgentOptions::builder()
        .backend(backend.kind)
        .cli_path(&backend.cli_path);

    // Set backend-specific defaults for safe testing
    match backend.kind {
        BackendKind::Codex => {
            builder = builder.codex(CodexOptions {
                approval_policy: Some("full-auto".to_string()),
                sandbox_mode: Some("read-only".to_string()),
            });
        }
        BackendKind::Cursor => {
            builder = builder.cursor(CursorOptions {
                force_approve: true,
                mode: None,
                trust_workspace: true,
            });
        }
        BackendKind::Claude => {}
        _ => {}
    }

    builder.build()
}

/// Human-readable name for a backend kind.
pub fn backend_name(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Claude => "Claude",
        BackendKind::Codex => "Codex",
        BackendKind::Cursor => "Cursor",
        _ => "Unknown",
    }
}

/// Find a CLI binary path from environment variable or PATH search.
fn find_cli_path(env_var: &str, binary_name: &str) -> Option<String> {
    // Check environment variable first
    if let Ok(path) = std::env::var(env_var) {
        if Path::new(&path).is_file() {
            return Some(path);
        }
    }

    // Search PATH
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(binary_name);
            if full.is_file() {
                return Some(full.to_string_lossy().to_string());
            }
        }
    }

    None
}
