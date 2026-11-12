# Upstream engine and portability delta

The formatter, parser, validator, configuration and date/time algorithms are
those of the upstream Tombi release `vX.Y.Z`. The dependencies in `Cargo.toml`
select branch `dprint/vX.Y.Z-headless` in `kjanat/tombi`, and `Cargo.lock` pins
its commit. That branch is based directly on the upstream release tag, not the
old fork's development history. It carries the portability commits from the
previous branch, then moves `serde_tombi`'s test-only Chrono dependency to
`dev-dependencies`.

Review the delta by comparing the upstream tag with the branch:
`https://github.com/tombi-toml/tombi/compare/vX.Y.Z...kjanat:tombi:dprint/vX.Y.Z-headless`.

Only eleven upstream files differ:

- The root `Cargo.toml` keeps the single Chrono version declaration and disables
  its default features at workspace level. `tombi-date-time/Cargo.toml` and
  `tombi-document-tree-syntax/Cargo.toml` inherit that version and add their
  required features. The `clock` feature remains enabled to preserve existing
  `chrono::Local` conversion types; it is separate from `wasmbind`. Formatting
  TOML date/time literals does not read the clock or the host's time zone.
- `tombi-lsp/Cargo.toml`, `tombi-test-lib/Cargo.toml`,
  `rust/serde_tombi/Cargo.toml` and `xtask/Cargo.toml` explicitly use
  `default-features = true` for Chrono, preserving their previous dependency
  profiles after the workspace change. `serde_tombi` uses Chrono only in tests,
  so its declaration is under `dev-dependencies`. This prevents its default
  browser bindings from entering the plugin build.
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

The plugin supplies an HTTP client that reads files downloaded by kprint.
Tombi's Wasm build has no disk cache. The artifact must import only dprint host
functions, not JavaScript, WASI or browser APIs.

## Why there is no post-link workaround

An intermediate experiment removed browser exports and ran Binaryen dead-code
elimination. Browser callbacks could remain in indirect-function tables, so that
was not a clean source-level solution. The implementation removes the unneeded
dependencies at source instead. There is no Binaryen dependency, export
rewriting, import stubbing, or copied formatter in this repository.

## Updating

Prefer upstream non-browser feature support when it becomes available. Until
then, rebase this small portability delta onto the chosen upstream release, and
push it as `dprint/vX.Y.Z-headless`. The `update` job of `tombi-upstream.yml`
(weekly, or run it by hand) then opens one pull request from the Beepboop App
that points every Tombi dependency at the newest such branch, updates
Cargo.lock, schema.json and `.github/tombi-cli.sha256`. Merging it releases the
plugin. Require native tests, strict native/WASM Clippy, and the unchanged
dprint-only import audit plus real CLI execution to pass. Do not expand the fork
into an independently maintained formatting engine.
