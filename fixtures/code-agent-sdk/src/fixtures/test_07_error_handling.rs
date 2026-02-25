//! Test: Error handling across backends.
//!
//! Verifies proper error reporting for:
//! - Invalid CLI path (all backends)
//! - NotConnected errors (all backends)
//! - Backend-specific error behavior
//!
//! These tests do not require any CLI to be installed.
//!
//! Covers: Error::CliNotFound, Error::NotConnected, error propagation

use code_agent_sdk::{
    query, AgentOptions, AgentSdkClient, BackendKind, CodexOptions, CursorOptions, Error,
};
use futures::StreamExt;

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Error handling (all backends) ===\n");

    test_invalid_cli_path_claude().await?;
    test_invalid_cli_path_codex().await?;
    test_invalid_cli_path_cursor().await?;
    test_not_connected().await?;

    println!("\n  All error handling tests passed");
    Ok(())
}

async fn test_invalid_cli_path_claude() -> Result<(), anyhow::Error> {
    println!("  --- Claude: invalid CLI path ---");

    let options = AgentOptions::builder()
        .backend(BackendKind::Claude)
        .cli_path("/nonexistent/claude-binary")
        .build();

    let mut stream = query("test", Some(options));
    let mut got_error = false;

    while let Some(msg_result) = stream.next().await {
        if let Err(e) = msg_result {
            got_error = true;
            println!("    Got expected error: {}", e);
            break;
        }
    }

    assert!(got_error, "Expected an error for invalid CLI path");
    println!("    Passed");
    Ok(())
}

async fn test_invalid_cli_path_codex() -> Result<(), anyhow::Error> {
    println!("  --- Codex: invalid CLI path ---");

    let options = AgentOptions::builder()
        .backend(BackendKind::Codex)
        .cli_path("/nonexistent/codex-binary")
        .codex(CodexOptions {
            approval_policy: Some("full-auto".to_string()),
            sandbox_mode: None,
        })
        .build();

    let mut stream = query("test", Some(options));
    let mut got_error = false;

    while let Some(msg_result) = stream.next().await {
        if let Err(e) = msg_result {
            got_error = true;
            println!("    Got expected error: {}", e);
            break;
        }
    }

    assert!(got_error, "Expected an error for invalid CLI path");
    println!("    Passed");
    Ok(())
}

async fn test_invalid_cli_path_cursor() -> Result<(), anyhow::Error> {
    println!("  --- Cursor: invalid CLI path ---");

    let options = AgentOptions::builder()
        .backend(BackendKind::Cursor)
        .cli_path("/nonexistent/agent-binary")
        .cursor(CursorOptions {
            force_approve: true,
            mode: None,
            trust_workspace: true,
        })
        .build();

    let mut stream = query("test", Some(options));
    let mut got_error = false;

    while let Some(msg_result) = stream.next().await {
        if let Err(e) = msg_result {
            got_error = true;
            println!("    Got expected error: {}", e);
            break;
        }
    }

    assert!(got_error, "Expected an error for invalid CLI path");
    println!("    Passed");
    Ok(())
}

async fn test_not_connected() -> Result<(), anyhow::Error> {
    println!("  --- NotConnected error ---");

    // Claude backend
    {
        let client = AgentSdkClient::new(None, None);
        let result = client.get_server_info().await;
        match result {
            Err(Error::NotConnected) => {
                println!("    Claude: correctly returns NotConnected");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    // Codex backend
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .build();
        let client = AgentSdkClient::new(Some(options), None);
        let result = client.get_server_info().await;
        match result {
            Err(Error::NotConnected) => {
                println!("    Codex: correctly returns NotConnected");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    // Cursor backend
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Cursor)
            .build();
        let client = AgentSdkClient::new(Some(options), None);
        let result = client.get_server_info().await;
        match result {
            Err(Error::NotConnected) => {
                println!("    Cursor: correctly returns NotConnected");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    println!("    Passed");
    Ok(())
}
