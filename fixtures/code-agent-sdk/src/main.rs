//! Fixture test runner for multi-backend code-agent-sdk.
//!
//! Run individual fixtures: cargo run -p code-agent-sdk-fixtures -- test_01
//! Run all:                 cargo run -p code-agent-sdk-fixtures -- all
//! Run offline only:        cargo run -p code-agent-sdk-fixtures -- offline

use std::env;

mod fixtures;

fn main() {
    let args: Vec<String> = env::args().collect();
    let test_name = args.get(1).map(|s| s.as_str()).unwrap_or("test_01");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    let result = rt.block_on(async {
        match test_name {
            "test_01" => fixtures::test_01_basic_query::run().await,
            "test_02" => fixtures::test_02_query_with_options::run().await,
            "test_02_select" => fixtures::test_02_query_with_options::run_backend_selection().await,
            "test_03" => fixtures::test_03_multi_turn_session::run().await,
            "test_04" => fixtures::test_04_codex_options::run().await,
            "test_05" => fixtures::test_05_cursor_options::run().await,
            "test_06" => fixtures::test_06_backend_validation::run().await,
            "test_07" => fixtures::test_07_error_handling::run().await,
            "test_08" => fixtures::test_08_message_types::run().await,
            "offline" => run_offline().await,
            "all" => run_all().await,
            _ => {
                eprintln!("Unknown fixture: {}", test_name);
                eprintln!("Available: test_01..test_08, test_02_select, offline, all");
                Err(anyhow::anyhow!("Unknown fixture"))
            }
        }
    });

    if let Err(e) = result {
        eprintln!("Fixture failed: {}", e);
        std::process::exit(1);
    }
}

/// Run offline tests that do not require any CLI to be installed.
async fn run_offline() -> Result<(), anyhow::Error> {
    macro_rules! run {
        ($name:expr, $fn:path) => {
            println!("\n>>> Running {} <<<", $name);
            $fn().await?;
            println!(">>> {} passed <<<", $name);
        };
    }

    run!("test_06_backend_validation", fixtures::test_06_backend_validation::run);
    run!("test_07_error_handling", fixtures::test_07_error_handling::run);

    println!("\nAll offline fixtures passed");
    Ok(())
}

/// Run all fixtures. Online tests are skipped if CLI is not available.
async fn run_all() -> Result<(), anyhow::Error> {
    macro_rules! run {
        ($name:expr, $fn:path) => {
            println!("\n>>> Running {} <<<", $name);
            $fn().await?;
            println!(">>> {} passed <<<", $name);
        };
    }

    // Offline tests (always run)
    run!("test_06_backend_validation", fixtures::test_06_backend_validation::run);
    run!("test_07_error_handling", fixtures::test_07_error_handling::run);

    // Online tests (skip gracefully if CLI not available)
    run!("test_01_basic_query", fixtures::test_01_basic_query::run);
    run!("test_02_query_with_options", fixtures::test_02_query_with_options::run);
    run!("test_02_backend_selection", fixtures::test_02_query_with_options::run_backend_selection);
    run!("test_03_multi_turn_session", fixtures::test_03_multi_turn_session::run);
    run!("test_04_codex_options", fixtures::test_04_codex_options::run);
    run!("test_05_cursor_options", fixtures::test_05_cursor_options::run);
    run!("test_08_message_types", fixtures::test_08_message_types::run);

    println!("\nAll fixtures passed");
    Ok(())
}
