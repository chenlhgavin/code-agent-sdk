# Code Agent SDK Makefile

.PHONY: build test fmt clippy audit fixtures fixtures-test

build:
	cargo build

test:
	cargo test
	cargo test -- --ignored 2>/dev/null || true

fmt:
	cargo +nightly fmt 2>/dev/null || cargo fmt

clippy:
	cargo clippy -- -D warnings -W clippy::pedantic

audit:
	cargo audit

# 运行 fixtures（需要 ANTHROPIC_API_KEY）
fixtures:
	cargo run -p code-agent-sdk-fixtures -- all

# 运行单个 fixture，例如: make fixtures-test FIXTURE=test_01
fixtures-test:
	cargo run -p code-agent-sdk-fixtures -- $(or $(FIXTURE),test_01)

all: fmt build test clippy
