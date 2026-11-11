# dprint-plugin-tombi

A dprint TOML formatter using the same Rust formatting engine as
[Tombi's `tombi-wasm-lib`](https://tombi-toml.github.io/tombi/docs/wasm/tombi-wasm-lib).
The engine is Tombi **v1.11.1**. The plugin exposes dprint's WASM ABI; it does
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

### Tombi's configuration file

With [kprint](https://github.com/kjanat/kprint) 0.60.5 or later, the plugin
reads formatting options from Tombi's configuration file. The first of these
next to the dprint configuration file is used, in Tombi's order: `.tombi.toml`,
`tombi.toml`, `.config/tombi.toml`, `pyproject.toml` with a `[tool.tombi]`
table. Its `toml-version`, `format.rules` and `[[schemas]]` apply; the plugin's
own properties take precedence over them, and they over dprint's globals.

`configFile` names another file, or the part of one, or turns it off:

```jsonc
{
  "toml": {
    "configFile": "configs/tombi.toml", // or "pyproject.toml#tool.tombi", or false
  },
}
```

Another dprint passes the plugin no file: configure it with its own properties
there. `configFile` is diagnosed, rather than ignored.

### Remote configuration and inheritance

With kprint's remote configuration support, `configFile` also accepts a URL:

```json
{
  "toml": {
    "configFile": "https://example.com/formatting/tombi.toml"
  }
}
```

That file can use Tombi's `extends`:

```toml
extends = ["../shared/tombi.toml"]

[format.rules]
line-width = 100
```

Kprint downloads the files through its HTTP cache. The plugin applies Tombi's
inheritance rules, then applies any options set directly under `toml` in the
kprint configuration. Relative URLs resolve beside the file that declares them.
After a redirect, they resolve beside the destination URL.

Local configuration files can extend local or remote files. Remote files can
extend HTTP(S) URLs. Missing files, inheritance cycles and invalid `extends`
settings stop formatting with an error.

This support is on the development branches in
[kprint #61](https://github.com/kjanat/kprint/pull/61) and
[this plugin's #21](https://github.com/kjanat/dprint-plugin-tombi/pull/21).

### Per file

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

Kprint supplies schemas listed in `[[schemas]]`, including files referenced by
`$ref`. It reads local files and downloads HTTP(S) URLs. Relative local schema
paths use the selected Tombi configuration's directory, including paths from
inherited local configurations. For `.config/tombi.toml`, Tombi uses the
directory above `.config`. Relative URLs in remote configurations use that
file's URL. Local files must be inside the directory containing the kprint
configuration. Changes to a local schema invalidate the cached plugin
configuration, including in the language server.

This requires kprint's `get_config_schema_paths` support. Older hosts report a
configuration error for local schemas. The plugin also uses the schemas built
into Tombi. Custom schema catalogs are not loaded.

The `schema` settings in `tombi.toml` apply too. For example,
`schema.format.rules.table-keys-order.enabled = false` turns off schema-based
key sorting, as it does in `tombi format`.

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

Kprint supplies the files before formatting starts. The plugin reports an error
if Tombi requests asynchronous work while formatting. Tests exercise this error
and run the compiled Wasm plugin through kprint.

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

Compare the built plugin with Tombi 1.11.1 using the same configuration and
local schemas:

```sh
node scripts/schema-parity.mjs
node scripts/remote-config-parity.mjs
```

This needs a kprint build with schema-file support. Set `KPRINT_BIN` and
`TOMBI_BIN` to test specific executables. The comparison covers table order,
arrays of tables, relative schema references, schema changes, disabled sorting
and a second formatting pass.

Commit `Cargo.lock` and the generated `schema.json`. The schema generator sorts
objects recursively for stable output. CI rejects missing or stale generated
files. All Tombi dependencies select the same fork branch. Update their locked
commit together, regenerate the schema, and rerun the native and WASM checks.

Releases use bare semver tags matching `Cargo.toml`, and publish the exact
`plugin.wasm` and `schema.json` that passed CI. Published versions are
immutable; fix a bad release with a new version, not a replaced binary.

## License

MIT. Tombi's upstream copyright and license are included in `LICENSE-TOMBI`.
