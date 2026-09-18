# Narrow Tombi portability patches

Source: Tombi v1.5.5, commit `b598e6fb9aab62cd5c8677ac832dd2a9c3b32e36`.
License: MIT; see `../LICENSE-TOMBI`.

These are the only vendored crates. The formatter, parser, validator,
configuration types and schema store are still loaded from the pinned upstream
revision. Do not expand this into a second formatter fork.

## `tombi-hashmap`

`src/lib.rs` is unchanged from upstream. Upstream's manifest enables browser
`getrandom/wasm_js` on every WASM target. This manifest instead selects ahash's
supported `std` + `no-rng` profile on WASM, preserving normal runtime seeding on
native targets. This affects hash-table seeding only. It is not a replacement
randomness API, and must never be reused for cryptographic randomness. Fixed
hash seeds have weaker adversarial collision resistance than secret per-process
seeds; the production formatter remains inside dprint's sandbox.

## `tombi-future`

The boxing and native-spawn implementations are preserved. The browser dependency
`wasm-bindgen-futures`, `TaskHandle`, and `spawn` are gated by the existing `wasm`
feature instead of being unconditionally enabled for `target_arch = "wasm32"`.
No fake scheduler is supplied. The dprint dependency profile does not enable that
feature, and would fail to compile if it started requiring browser-only spawning.
Workspace manifest values have been made explicit for the local patch crates.

## Removing the patches

When upstream supports non-browser WASM through its dependency features, replace
these patches with those upstream feature settings, regenerate Cargo.lock, and
require `scripts/wasm-smoke.mjs` to pass unchanged. Never whitelist the offending
browser imports or provide dummy functions just to make instantiation succeed.
