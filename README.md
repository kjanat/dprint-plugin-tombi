# [dprint-plugin-tombi][repo]

A [dprint][dprint] plugin for formatting TOML files using [Tombi][tombi].

## Installation

```bash
dprint config add kjanat/tombi
```

Add to your `dprint.json`:

<!-- dprint-ignore-start -->

```json
{
  "plugins": [
    // See https://github.com/kjanat/dprint-plugin-tombi/releases
    "https://plugins.dprint.dev/kjanat/tombi-x.x.x.wasm"
  ]
}
```

<!-- dprint-ignore-end -->

## Configuration

```json
{
  "toml": {
    "lineWidth": 120,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineEnding": "lf",
    "stringQuoteStyle": "double",
    "dateTimeDelimiter": "T",
    "indentSubTables": false,
    "indentTableKeyValuePairs": false,
    "keyValueEqualsSignAlignment": false,
    "trailingCommentAlignment": false,
    "tomlVersion": "v1.0.0"
  }
}
```

## Building

```bash
# Native (for testing)
cargo build --release

# Wasm (for dprint)
cargo build --release --target=wasm32-unknown-unknown
```

## Development

This plugin wraps Tombi's formatter. Key dependencies:

- `tombi-formatter` - Core formatting logic
- `tombi-config` - Configuration types
- `tombi-parser` - TOML parsing
- `dprint-core` - dprint plugin interface

## License

[MIT][license]

<!-- link definitions -->

[dprint]: https://dprint.dev/
[license]: https://github.com/kjanat/dprint-plugin-tombi/blob/master/LICENSE
[repo]: https://github.com/kjanat/dprint-plugin-tombi "Go to: GitHub repository"
[tombi]: https://github.com/tombi-toml/tombi

<!--[actions:release]: https://github.com/kjanat/dprint-plugin-tombi/actions/workflows/release.yml-->
<!--[badge:ci:release]: https://img.shields.io/github/actions/workflow/status/kjanat/dprint-plugin-tombi/release.yml?logo=githubactions&logoColor=2088FF&logoSize=auto&label=Release&labelColor=181717-->
<!--[badge:jsr]: https://img.shields.io/jsr/v/@kjanat/dprint-tombi?logo=jsr&logoColor=black&logoSize=auto&label=&labelColor=F7DF1E&color=black-->
<!--[badge:npm]: https://img.shields.io/npm/v/@kjanat/dprint-tombi?logo=npm&logoColor=white&logoSize=auto&label=&labelColor=CB3837&color=black-->
<!--[contributing]: https://github.com/kjanat/dprint-plugin-tombi/blob/master/CONTRIBUTING.md-->
<!--[npm]: https://www.npmjs.com/package/@kjanat/dprint-tombi-->
<!--[jsr]: https://jsr.io/@kjanat/dprint-tombi-->
