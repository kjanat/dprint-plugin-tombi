# AGENTS.md — dprint-plugin-tombi

## Architecture

This is a Rust → `wasm32-unknown-unknown` dprint plugin wrapping the same
formatter engine as `tombi-wasm-lib`. The browser WASM ABI is not compatible
with dprint. Do not add a JS runtime, process plugin, copied formatter or
post-link import stubs. All Tombi dependencies use the same fork branch.
`Cargo.lock` records the resolved commit.

The dependency is based directly on upstream v1.11.1 with a small portability
delta in the existing Tombi fork. See UPSTREAM.md for provenance and
hash-seeding tradeoffs. Keep the formatter/parser algorithms unchanged. Prefer
upstream non-browser feature support when available rather than growing the
fork.

Keep the existing configuration key `toml`. `src/configuration.rs` discovers and
validates upstream `FormatRules` through serde, exposing keys in camelCase.
Unset rules retain upstream defaults; plugin keys override inherited globals.
Invalid/unknown configuration produces diagnostics, not silent coercions.

Kprint reads configuration files, follows the references declared by the plugin,
and downloads remote files through its HTTP cache. The plugin receives their
contents and uses Tombi's loader to apply `extends`. Its HTTP client reads only
these supplied files. Wasm builds use Tombi's in-memory filesystem and have no
disk cache. `src/handler.rs` polls the formatter once and reports an error if it
requests asynchronous work. Test the compiled Wasm plugin as well as native
code.

`cargo wasm --locked` produces the distributable artifact directly. There is no
Binaryen finalization, export rewriting, or Node build dependency. The Node
integration-test script inspects that unmodified artifact and executes it in
dprint; Node is not required to build or run the plugin.

## Verification

```sh
cargo test --locked --all-features
cargo lint
cargo lint-wasm
cargo fmt --all -- --check
cargo run --locked --bin generate-schema --features schema
cargo wasm --locked
node scripts/wasm-smoke.mjs
node scripts/schema-parity.mjs
node scripts/remote-config-parity.mjs
```

Run strict Clippy on all native targets/features and the WASM library. Fix
findings instead of weakening lint policy. The integration script needs dprint
on PATH or `DPRINT_BIN`; it rejects non-dprint imports and tests the real CLI.
Native tests alone are not evidence of WASM compatibility.

Run `schema-parity.mjs` with Tombi 1.11.1 and a kprint build that supports
schema files. `KPRINT_BIN` and `TOMBI_BIN` select those executables. This test
compares their output using the same configuration and schemas. The remote
comparison also checks relative URLs, redirects, inheritance, download caching
and invalid references.

## Generated files and release

Commit `Cargo.lock` and `schema.json`; CI checks they are tracked and unchanged
by generation. Never hand-edit schema properties. Defaults, descriptions, enums
and numeric constraints come from upstream Rust types. Keep the runtime and
schema key sets identical, including supported version aliases.

URLs derive from `package.repository` and `package.version` through `build.rs`.
Use bare semver release tags and the artifact name `plugin.wasm`. Publish the
exact artifact that passed CI, not a separately rebuilt binary. Never replace
existing release artifacts.

The public dprint proxy requires a public repository and a published release. Do
not describe an unreleased/private build as publicly installable.

## Style

Rust edition 2024. English comments. Use dprint's `FormatError` at the plugin
API boundary and `anyhow` for developer tooling. Reject invalid UTF-8 and
unsupported ranges. Preserve no-change/idempotence and cancellation checks. Keep
upstream licensing notices.
