# AGENTS.md - dprint-plugin-tombi

## Overview

dprint plugin wrapping Tombi TOML formatter for use with dprint.

## Build & Test Commands

- `cargo build` - Build native (for testing)
- `cargo build --release --target=wasm32-unknown-unknown` - Build wasm plugin
- `cargo test` - Run tests
- `cargo clippy` - Lint
- `cargo fmt` - Format
- `cargo run --bin generate-schema --features schema` - Generate schema.json

## Code Style

- **Rust edition**: 2024
- **Comments**: English only
- **Errors**: Use `anyhow` for error handling

## Project Structure

- `src/lib.rs` - Plugin entry point, wasm macro
- `src/handler.rs` - dprint SyncPluginHandler implementation, format logic
- `src/configuration.rs` - Configuration struct with schemars derive for schema
  generation
- `src/bin/generate_schema.rs` - Binary to auto-generate schema.json (generated
  at release, not committed)

## Key Dependencies

- `dprint-core` - dprint plugin interface (wasm feature)
- `tombi-formatter` - Core TOML formatting
- `tombi-config` - Tombi configuration types
- `tombi-parser` - TOML parsing
- `tombi-schema-store` - Schema resolution (required by formatter API, used
  minimally)
- `tombi-toml-version` - TOML version enum
- `schemars` - JSON Schema generation (optional, schema feature)

## Architecture Notes

- `tombi-schema-store` is architecturally required by `Formatter::new()` but
  used with empty store
- Native builds use tokio runtime for async; wasm async handling is TODO
- Configuration maps dprint global config (lineWidth, useTabs, etc.) to Tombi
  equivalents
- Schema is auto-generated from Configuration struct - update config, regenerate
  schema

## CI/CD

- `.github/workflows/ci.yml` - Test, lint, schema check, wasm build on PR/push
- `.github/workflows/release.yml` - Tag-triggered release with plugin.wasm +
  schema.json
  - Validates git tag matches Cargo.toml version
  - Generates schema.json fresh with version from Cargo.toml
  - Asset named `plugin.wasm` for dprint proxy service compatibility

## Wasm Status

Currently blocked on upstream tombi tokio dependencies not supporting
wasm32-unknown-unknown. Requires feature flags in tombi crates for proper wasm
support.
