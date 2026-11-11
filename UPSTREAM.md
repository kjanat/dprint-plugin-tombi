# Upstream engine and portability delta

The formatter, parser, validator, configuration and date/time algorithms are
Tombi v1.11.1, upstream commit `273ace37a1f7b86bc845271d91dfe285b0539e93`.

The dependency uses commit `6e52da2e17602c2d70f6b18f1ca68f029d20f6d2` in the
existing `kjanat/tombi` fork, on branch `dprint/v1.11.1-headless`. That branch
is based directly on the upstream release, not the old fork's development
history. It applies the four portability commits from `dprint/v1.9.8-headless`
to Tombi v1.11.1.

[Review the complete eleven-file portability delta](https://github.com/kjanat/tombi/compare/273ace37a1f7b86bc845271d91dfe285b0539e93...6e52da2e17602c2d70f6b18f1ca68f029d20f6d2).

Only eleven upstream files differ:

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
- `tombi-schema-store/Cargo.toml` asks for Tokio's `rt` feature, which its
  `tokio::task_local!` state needs. Native builds got it through feature
  unification from other crates; the headless Wasm build has none and didn't
  compile.

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
then, rebase this small portability delta onto the chosen upstream release,
point all Tombi dependencies at the resulting immutable commit, and regenerate
Cargo.lock and schema.json. Require native tests, strict native/WASM Clippy, and
the unchanged dprint-only import audit plus real CLI execution to pass. Do not
expand the fork into an independently maintained formatting engine.
