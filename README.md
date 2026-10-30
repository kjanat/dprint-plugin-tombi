# dprint-plugin-tombi

A dprint TOML formatter using the same Rust formatting engine as
[Tombi's `tombi-wasm-lib`](https://tombi-toml.github.io/tombi/docs/wasm/tombi-wasm-lib).
The engine is Tombi **v1.9.8**. The plugin exposes dprint's WASM ABI; it does
not embed a JavaScript runtime or call a separate Tombi executable.

## Install

```sh
dprint add kjanat/tombi
```

Then configure it under the **`toml`** key, as in the original plugin, not
`tombi`:

```jsonc
{
  "toml": {
    "lineWidth": 100,
    "indentWidth": 2,
    "lineEnding": "auto"
  }
}
```

Do not enable another TOML plugin ahead of this one unless that routing is
intentional.

## Configuration

Tombi's `format.rules` options are exposed directly in camelCase: `line-width`
becomes `lineWidth`, `key-quote-style` becomes `keyQuoteStyle`, and so on. The
adapter validates values against the actual upstream Rust types, rather than
maintaining a second set of enums, ranges and defaults. Invalid or unknown
options produce dprint configuration diagnostics.

The generated `schema.json` is the complete configuration reference, including
upstream descriptions and constraints. Besides formatting rules, `tomlVersion`
accepts `v1.0.0` or `v1.1.0`; the old unprefixed spellings remain accepted.

Unset rules retain Tombi's defaults. dprint globals are inherited where
provided: `lineWidth`, `indentWidth`, `useTabs` → `indentStyle`, and
`newLineKind` → `lineEnding`. An explicit plugin option takes precedence. `null`
resets an optional rule to Tombi's default, including removing an inherited
line-width limit. Values outside Tombi's supported range are diagnosed, never
truncated.

Per-file `overrides` and additive/negated `associations` are handled by dprint.
For example:

```jsonc
{
  "toml": {
    "stringQuoteStyle": "single",
    "associations": ["**/*.toml-extra", "!**/generated.toml"],
    "overrides": [
      { "files": "**/Cargo.toml", "stringQuoteStyle": "double" }
    ]
  }
}
```

## What is and is not reused

The browser package's binary cannot simply be renamed to `plugin.wasm`: its
`wasm-bindgen` exports and JavaScript imports are not dprint's plugin protocol.
Instead, this adapter compiles the upstream formatter/configuration/schema-store
crates into one dprint WASM module. No formatting algorithm is copied or
rewritten. A branch of the existing Tombi fork contains a small portability
delta: disable Chrono's JavaScript backend, avoid a browser RNG backend for hash
tables, and make browser task spawning opt-in. No formatter, parser, validator
or date/time algorithm changes are included. See [UPSTREAM.md](UPSTREAM.md) for
the exact base commit, dependency revision and tradeoffs.

The build is Rust-only. `cargo wasm --locked` produces the usable plugin without
post-link rewriting, Binaryen, a Node build pipeline, or fake host functions.
Node is used only by the integration test harness; it is not needed to build or
run the plugin.

The production plugin is intentionally sandboxed. It does not discover
`tombi.toml` or `pyproject.toml`, read local JSON Schema files, fetch remote
schemas/catalogs, or expose Tombi's separate lint API. It uses an offline,
cache-disabled schema store with the schemas built into Tombi (its catalog, and
the schemas for `Cargo.toml`, `pyproject.toml` and `tombi.toml`), made once per
`schemas` configuration and kept. A schema can decide how a file formats, as in
the CLI: `tombi.toml`'s says TOML v1.1.0, which keeps a multi-line inline table.
Other schemas can't be fetched, so formatting that depends on them may differ
from the CLI.

`schemas` takes Tombi's `[[schemas]]` with camelCase keys, ahead of the built-in
ones. A remote schema Tombi has built in is used from its built-in copy, so an
entry's settings apply as they do in the CLI. For example, as a `tombi.toml` can
say that Cargo reads TOML 1.0:

```jsonc
{
  "toml": {
    "schemas": [{
      "path": "https://www.schemastore.org/cargo.json",
      "include": ["Cargo.toml", "**/Cargo.toml"],
      "tomlVersion": "v1.0.0"
    }]
  }
}
```

Formatting is whole-document only; range requests return an explicit error.
Invalid UTF-8 is rejected instead of being replaced with replacement characters.
Already-formatted input returns dprint's no-change result.

Tombi's formatter API is async, but the offline path must finish in one poll.
The adapter checks that contract and returns an error on `Pending`; it never
spins, blocks a thread, or invents a JavaScript event loop. CI tests both the
guard and the actual WASM artifact so an upstream change cannot silently break
it.

## Development

```sh
cargo test --locked --all-features
cargo lint
cargo lint-wasm
cargo fmt --all -- --check
cargo run --locked --bin generate-schema --features schema
cargo wasm --locked
node scripts/wasm-smoke.mjs
```

The last command needs dprint on `PATH` (or `DPRINT_BIN` pointing to it). CI
runs it with the latest kjanat/dprint and upstream dprint, and checks the
compiled module's imports before running formatting, dates, directives,
idempotence, diagnostics, associations and overrides through the CLI.

Commit `Cargo.lock` and the generated `schema.json`. The schema generator sorts
objects recursively for stable output. CI rejects missing or stale generated
files. When updating Tombi, update every Tombi revision together, regenerate the
lockfile and schema, and rerun the native and WASM gates.

Releases use bare semver tags matching `Cargo.toml`, and publish the exact
`plugin.wasm` and `schema.json` that passed CI. Published versions are
immutable; fix a bad release with a new version, not a replaced binary.

## License

MIT. Tombi's upstream copyright and license are included in `LICENSE-TOMBI`.
