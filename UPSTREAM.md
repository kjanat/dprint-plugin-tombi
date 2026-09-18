# Upstream engine and portability delta

The formatter, parser, validator, configuration and date/time algorithms are
Tombi v1.5.5, upstream commit `b598e6fb9aab62cd5c8677ac832dd2a9c3b32e36`.

The dependency is pinned to `0428b31911e4623574b10c1d4fbc9f63a63999d6` in the
existing `kjanat/tombi` fork, on branch `dprint/v1.5.5-headless`. That branch is
based directly on the upstream release, not the old fork's development history.

[Review the complete ten-file portability delta](https://github.com/kjanat/tombi/compare/b598e6fb9aab62cd5c8677ac832dd2a9c3b32e36...0428b31911e4623574b10c1d4fbc9f63a63999d6).

Only ten upstream files differ:

- The root `Cargo.toml` keeps the single Chrono version declaration and disables
  its default features at workspace level. `tombi-date-time/Cargo.toml` and
  `tombi-document-tree-syntax/Cargo.toml` inherit that version and add their
  required features. The `clock` feature remains enabled to preserve existing
  `chrono::Local` conversion types; it is separate from `wasmbind`. Formatting
  TOML date/time literals does not read the clock or the host's time zone.
- `tombi-lsp/Cargo.toml`, `tombi-test-lib/Cargo.toml`,
  `rust/serde_tombi/Cargo.toml` and `xtask/Cargo.toml` explicitly retain
  `default-features = true` for Chrono, preserving their previous dependency
  profiles after the workspace change.
- `tombi-hashmap/Cargo.toml` selects ahash's supported `std` + `no-rng` profile
  for WASM, while preserving ordinary native seeding. This changes hash-table
  seeding, not a randomness API. Fixed seeds are weaker against adversarial
  collision attacks than secret per-process seeds; never use this as a
  cryptographic randomness provider.
- `tombi-future/Cargo.toml` and `tombi-future/src/on_wasm.rs` make browser task
  spawning opt-in through the existing `wasm` feature. `wasm-send` still opts
  into that backend. The headless formatter does not enable it; no fake task
  scheduler is supplied.

The plugin disables schema-store HTTP features and disk caching. The artifact
must import only dprint host functions, not JavaScript, WASI or browser APIs.

## Why there is no post-link workaround

An intermediate experiment removed browser exports and ran Binaryen dead-code
elimination. Browser callbacks could remain in indirect-function tables, so that
was not a clean source-level solution. The implementation removes the unneeded
dependencies at source instead. There is no Binaryen dependency, export
rewriting, import stubbing, or copied formatter in this repository.

## Updating

Prefer upstream non-browser feature support when it becomes available. Until
then, rebase this small portability delta onto the chosen upstream release, pin
all Tombi dependencies to the resulting immutable commit, and regenerate
Cargo.lock and schema.json. Require native tests, strict native/WASM Clippy, and
the unchanged dprint-only import audit plus real CLI execution to pass. Do not
expand the fork into an independently maintained formatting engine.
