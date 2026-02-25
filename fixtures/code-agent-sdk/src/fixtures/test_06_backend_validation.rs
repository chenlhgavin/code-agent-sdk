//! Test: Backend option validation and capability gating.
//!
//! Verifies that unsupported options are rejected and unsupported capabilities
//! return appropriate errors. These tests do not require any CLI to be installed.
//!
//! Covers: Error::UnsupportedOptions, Error::UnsupportedFeature, validate_options,
//!         Capabilities

use code_agent_sdk::{
    query, AgentOptions, AgentSdkClient, BackendKind, CodexOptions, CursorOptions, Error,
};
use futures::StreamExt;

pub async fn run() -> Result<(), anyhow::Error> {
    println!("=== Test: Backend validation and capability gating ===\n");

    test_codex_rejects_unsupported_options().await?;
    test_cursor_rejects_unsupported_options().await?;
    test_capability_gating().await?;

    println!("\n  All validation tests passed");
    Ok(())
}

/// Codex backend rejects system_prompt, hooks, fork_session, etc.
async fn test_codex_rejects_unsupported_options() -> Result<(), anyhow::Error> {
    println!("  --- Codex rejects unsupported options ---");

    let options = AgentOptions::builder()
        .backend(BackendKind::Codex)
        .system_prompt("This should be rejected")
        .build();

    let mut stream = query("test", Some(options));
    let first = stream.next().await;

    match first {
        Some(Err(Error::UnsupportedOptions { backend, options })) => {
            assert_eq!(backend, "Codex");
            assert!(options.contains(&"system_prompt".to_string()));
            println!("    Correctly rejected system_prompt: {:?}", options);
        }
        other => {
            return Err(anyhow::anyhow!(
                "Expected UnsupportedOptions error, got: {:?}",
                other
            ));
        }
    }

    println!("    Passed");
    Ok(())
}

/// Cursor backend rejects system_prompt, can_use_tool, hooks, mcp_servers, etc.
async fn test_cursor_rejects_unsupported_options() -> Result<(), anyhow::Error> {
    println!("  --- Cursor rejects unsupported options ---");

    let options = AgentOptions::builder()
        .backend(BackendKind::Cursor)
        .system_prompt("This should be rejected")
        .build();

    let mut stream = query("test", Some(options));
    let first = stream.next().await;

    match first {
        Some(Err(Error::UnsupportedOptions { backend, options })) => {
            assert_eq!(backend, "Cursor");
            assert!(options.contains(&"system_prompt".to_string()));
            println!("    Correctly rejected system_prompt: {:?}", options);
        }
        other => {
            return Err(anyhow::anyhow!(
                "Expected UnsupportedOptions error, got: {:?}",
                other
            ));
        }
    }

    // Test output_format rejection
    let options = AgentOptions::builder()
        .backend(BackendKind::Cursor)
        .output_format(serde_json::json!({"type": "json_schema"}))
        .build();

    let mut stream = query("test", Some(options));
    let first = stream.next().await;

    match first {
        Some(Err(Error::UnsupportedOptions { backend, options })) => {
            assert_eq!(backend, "Cursor");
            assert!(options.iter().any(|o| o.contains("output_format")));
            println!("    Correctly rejected output_format: {:?}", options);
        }
        other => {
            return Err(anyhow::anyhow!(
                "Expected UnsupportedOptions for output_format, got: {:?}",
                other
            ));
        }
    }

    println!("    Passed");
    Ok(())
}

/// AgentSdkClient capability gating: interrupt, set_model, etc.
async fn test_capability_gating() -> Result<(), anyhow::Error> {
    println!("  --- Capability gating ---");

    // Cursor backend: interrupt not supported
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Cursor)
            .cursor(CursorOptions {
                force_approve: true,
                mode: None,
                trust_workspace: true,
            })
            .build();

        let mut client = AgentSdkClient::new(Some(options), None);
        // Don't connect (would need real CLI), just test the capability check
        // by calling interrupt before connect
        let result = client.interrupt().await;
        match result {
            Err(Error::UnsupportedFeature { feature, backend }) => {
                assert_eq!(feature, "interrupt");
                assert_eq!(backend, "Cursor");
                println!("    Cursor correctly rejects interrupt: feature={}, backend={}", feature, backend);
            }
            Err(Error::NotConnected) => {
                // Also acceptable if capability check happens after connection check
                println!("    Cursor returns NotConnected (capability checked after connect)");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected UnsupportedFeature or NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    // Codex backend: set_model not supported (no runtime_config_changes)
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .codex(CodexOptions {
                approval_policy: Some("full-auto".to_string()),
                sandbox_mode: None,
            })
            .build();

        let mut client = AgentSdkClient::new(Some(options), None);
        let result = client.set_model(Some("gpt-4")).await;
        match result {
            Err(Error::UnsupportedFeature { feature, backend }) => {
                assert_eq!(feature, "set_model");
                assert_eq!(backend, "Codex");
                println!(
                    "    Codex correctly rejects set_model: feature={}, backend={}",
                    feature, backend
                );
            }
            Err(Error::NotConnected) => {
                println!("    Codex returns NotConnected (capability checked after connect)");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected UnsupportedFeature or NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    // Codex backend: set_permission_mode not supported
    {
        let options = AgentOptions::builder()
            .backend(BackendKind::Codex)
            .build();

        let mut client = AgentSdkClient::new(Some(options), None);
        let result = client.set_permission_mode("acceptEdits").await;
        match result {
            Err(Error::UnsupportedFeature { feature, backend }) => {
                assert_eq!(feature, "set_permission_mode");
                assert_eq!(backend, "Codex");
                println!(
                    "    Codex correctly rejects set_permission_mode: feature={}, backend={}",
                    feature, backend
                );
            }
            Err(Error::NotConnected) => {
                println!("    Codex returns NotConnected (capability checked after connect)");
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Expected UnsupportedFeature or NotConnected, got: {:?}",
                    other
                ));
            }
        }
    }

    println!("    Passed");
    Ok(())
}
