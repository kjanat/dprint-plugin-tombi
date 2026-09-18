# Upstream engine and portability delta

The formatter, parser, validator, configuration and date/time algorithms are
Tombi v1.5.5, upstream commit `b598e6fb9aab62cd5c8677ac832dd2a9c3b32e36`.

The dependency is pinned to `b7d9225298676fde124c9cace387e0d846c64ec0` in the
existing `kjanat/tombi` fork, on branch `dprint/v1.5.5-headless`. Its parent is
exactly that upstream release commit, not the old fork's development history.

[Review the complete portability commit](https://github.com/kjanat/tombi/commit/b7d9225298676fde124c9cace387e0d846c64ec0).

Only five upstream files differ:

- `tombi-date-time/Cargo.toml` and `tombi-document-tree-syntax/Cargo.toml` select
  Chrono's calendar/serialization support without unnecessary clock/browser
  defaults. TOML date/time parsing does not need the current time.
- `tombi-hashmap/Cargo.toml` selects ahash's supported `std` + `no-rng` profile
  for WASM, while preserving ordinary native seeding. This changes hash-table
  seeding, not a randomness API. Fixed seeds are weaker against adversarial
  collision attacks than secret per-process seeds; never use this as a
  cryptographic randomness provider.
- `tombi-future/Cargo.toml` and `tombi-future/src/on_wasm.rs` make browser task
  spawning opt-in through the existing `wasm` feature. `wasm-send` still opts
  into that backend. The headless formatter does not enable it; no fake task
  scheduler is supplied.

The plugin disables schema-store HTTP features and disk caching. A real dprint
WASM host cannot supply JavaScript, filesystem or browser-clock APIs.

## Why there is no post-link workaround

An intermediate experiment removed browser exports and ran Binaryen dead-code
elimination. Browser callbacks could remain in indirect-function tables, so
that was not a clean source-level solution. The final implementation removes
the dependencies at source instead. There is no Binaryen dependency, export
rewriting, import stubbing, or copied formatter in this repository.

## Updating

Prefer upstream non-browser feature support when it becomes available. Until
then, rebase this small portability delta onto the chosen upstream release,
pin all Tombi dependencies to the resulting immutable commit, and regenerate
Cargo.lock and schema.json. Require native tests, strict native/WASM Clippy,
and the unchanged dprint-only import audit plus real CLI execution to pass.
Do not expand the fork into an independently maintained formatting engine.
