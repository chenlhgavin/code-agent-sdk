//! Fixture tests for multi-backend code-agent-sdk.
//!
//! Tests 01-03, 08 require real CLI backends (set via env or PATH).
//! Tests 04-05 require specific backends (Codex / Cursor).
//! Tests 06-07 are offline tests that validate error handling without CLIs.

pub mod helpers;
pub mod test_01_basic_query;
pub mod test_02_query_with_options;
pub mod test_03_multi_turn_session;
pub mod test_04_codex_options;
pub mod test_05_cursor_options;
pub mod test_06_backend_validation;
pub mod test_07_error_handling;
pub mod test_08_message_types;
