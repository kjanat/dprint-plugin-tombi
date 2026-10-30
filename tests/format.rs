use std::path::Path;

use dprint_core::configuration::{
    ConfigKeyMap, ConfigurationDiagnostic, GlobalConfiguration, NewLineKind,
};
use dprint_core::plugins::{
    FormatConfigId, FormatResult, NullCancellationToken, PluginResolveConfigurationResult,
    SyncFormatRequest, SyncPluginHandler,
};
use dprint_plugin_tombi::{Configuration, SCHEMA_URL, TombiPluginHandler, UPDATE_URL};
use serde_json::{Value, json};

fn resolve(
    value: Value,
    global: &GlobalConfiguration,
) -> PluginResolveConfigurationResult<Configuration> {
    let map: ConfigKeyMap = serde_json::from_value(value).unwrap();
    TombiPluginHandler::default().resolve_config(map, global)
}

fn format(bytes: &[u8], config: &Configuration) -> FormatResult {
    TombiPluginHandler::default().format(
        SyncFormatRequest {
            file_path: Path::new("test.toml"),
            file_bytes: bytes.to_vec(),
            config_id: FormatConfigId::uninitialized(),
            config,
            range: None,
            token: &NullCancellationToken,
        },
        |_| panic!("TOML formatting must not invoke another plugin"),
    )
}

#[test]
fn fixture_and_second_pass_are_stable() {
    let config = Configuration::default();
    let expected = include_bytes!("fixtures/expected.toml");
    assert_eq!(
        format(include_bytes!("fixtures/source.toml"), &config).unwrap(),
        Some(expected.to_vec())
    );
    assert!(format(expected, &config).unwrap().is_none());
}

#[test]
fn invalid_utf8_is_rejected_without_lossy_replacement() {
    assert!(format(&[b'a', b'=', 0xff], &Configuration::default()).is_err());
}

#[test]
fn syntax_errors_are_not_silently_formatted() {
    let error = format(b"a = [", &Configuration::default()).unwrap_err();
    assert_ne!(error.to_string(), "");
}

#[test]
fn empty_input_is_unchanged() {
    assert!(format(b"", &Configuration::default()).unwrap().is_none());
}

#[test]
fn enums_numbers_types_and_unknown_keys_are_diagnosed() {
    for (key, value) in [
        ("indentStyle", json!("banana")),
        ("lineEnding", json!("windows")),
        ("stringQuoteStyle", json!("curly")),
        ("lineWidth", json!(300)),
        ("indentWidth", json!(-1)),
        ("indentSubTables", json!("yes")),
        ("tomlVersion", json!("v8.0.0")),
        ("misspelledOption", json!(true)),
    ] {
        let resolved = resolve(json!({key: value}), &GlobalConfiguration::default());
        assert_eq!(resolved.diagnostics.len(), 1, "{key}");
        assert_eq!(resolved.diagnostics[0].property_name, key);
    }
}

#[test]
fn globals_are_inherited_and_local_options_win() {
    let global = GlobalConfiguration {
        line_width: Some(100),
        indent_width: Some(4),
        use_tabs: Some(true),
        new_line_kind: Some(NewLineKind::CarriageReturnLineFeed),
    };
    let inherited = resolve(json!({}), &global);
    assert_eq!(inherited.diagnostics, Vec::<ConfigurationDiagnostic>::new());
    let value = serde_json::to_value(&inherited.config).unwrap();
    assert_eq!(value["lineWidth"], 100);
    assert_eq!(value["indentWidth"], 4);
    assert_eq!(value["indentStyle"], "tab");
    assert_eq!(value["lineEnding"], "crlf");

    let overridden = resolve(
        json!({"lineWidth": 80, "indentStyle": "space", "lineEnding": "lf"}),
        &global,
    );
    assert_eq!(
        overridden.diagnostics,
        Vec::<ConfigurationDiagnostic>::new()
    );
    let value = serde_json::to_value(&overridden.config).unwrap();
    assert_eq!(value["lineWidth"], 80);
    assert_eq!(value["indentStyle"], "space");
    assert_eq!(value["lineEnding"], "lf");
}

#[test]
fn oversized_global_width_is_diagnosed_not_truncated() {
    let global = GlobalConfiguration {
        line_width: Some(300),
        ..GlobalConfiguration::default()
    };
    assert_eq!(
        resolve(json!({}), &global).diagnostics[0].property_name,
        "lineWidth"
    );
    assert_eq!(
        resolve(json!({"lineWidth": 80}), &global).diagnostics,
        Vec::<ConfigurationDiagnostic>::new()
    );
}

#[test]
fn explicit_null_restores_upstream_default() {
    let global = GlobalConfiguration {
        line_width: Some(80),
        ..GlobalConfiguration::default()
    };
    let resolved = resolve(json!({"lineWidth": null}), &global);
    assert_eq!(resolved.diagnostics, Vec::<ConfigurationDiagnostic>::new());
    assert!(resolved.config.rules.line_width.is_none());
}

#[test]
fn old_toml_version_aliases_remain_supported() {
    for version in ["1.0.0", "1.1.0"] {
        let resolved = resolve(
            json!({"tomlVersion": version}),
            &GlobalConfiguration::default(),
        );
        assert_eq!(resolved.diagnostics, Vec::<ConfigurationDiagnostic>::new());
        assert_eq!(
            serde_json::to_value(resolved.config.toml_version).unwrap(),
            format!("v{version}")
        );
    }
}

#[test]
fn upstream_default_rules_are_preserved() {
    let resolved = resolve(json!({}), &GlobalConfiguration::default());
    assert_eq!(resolved.diagnostics, Vec::<ConfigurationDiagnostic>::new());
    assert_eq!(
        resolved.config.rules,
        tombi_config::format::FormatRules::default()
    );
}

#[test]
fn explicit_crlf_and_auto_newlines_work() {
    for ending in ["crlf", "auto"] {
        let resolved = resolve(
            json!({"lineEnding": ending}),
            &GlobalConfiguration::default(),
        );
        assert_eq!(resolved.diagnostics, Vec::<ConfigurationDiagnostic>::new());
        assert_eq!(
            format(b"a=1\r\n", &resolved.config).unwrap(),
            Some(b"a = 1\r\n".to_vec())
        );
        assert!(format(b"a = 1\r\n", &resolved.config).unwrap().is_none());
    }
}

#[test]
fn metadata_matches_release_contract() {
    let info = TombiPluginHandler::default().plugin_info();
    assert_eq!(info.config_key, "toml");
    assert_eq!(info.help_url, env!("CARGO_PKG_REPOSITORY"));
    assert_eq!(info.config_schema_url, SCHEMA_URL);
    assert_eq!(info.update_url.as_deref(), Some(UPDATE_URL));
    assert_eq!(
        SCHEMA_URL,
        format!(
            "https://plugins.dprint.dev/kjanat/tombi/{}/schema.json",
            env!("CARGO_PKG_VERSION")
        )
    );
    let resolved = resolve(json!({}), &GlobalConfiguration::default());
    assert_eq!(resolved.file_matching.file_extensions, ["toml"]);
}

#[test]
fn range_requests_do_not_rewrite_the_entire_file() {
    let config = Configuration::default();
    let result = TombiPluginHandler::default().format(
        SyncFormatRequest {
            file_path: Path::new("test.toml"),
            file_bytes: b"a=1\nb=2\n".to_vec(),
            config_id: FormatConfigId::uninitialized(),
            config: &config,
            range: Some(0..3),
            token: &NullCancellationToken,
        },
        |_| unreachable!(),
    );
    assert!(result.unwrap_err().to_string().contains("range formatting"));
}

#[cfg(feature = "schema")]
#[test]
fn schema_and_runtime_have_exactly_the_same_keys() {
    let schema = dprint_plugin_tombi::configuration_schema().unwrap();
    let config = serde_json::to_value(Configuration::default()).unwrap();
    let properties = schema["properties"].as_object().unwrap();
    let config = config.as_object().unwrap();
    assert_eq!(properties.len(), config.len());
    for key in config.keys() {
        assert!(properties.contains_key(key), "missing schema for {key}");
        assert!(!key.contains('-'), "not camelCase: {key}");
    }
    assert_eq!(schema["$id"], SCHEMA_URL);
    assert_eq!(schema["additionalProperties"], false);
    assert!(schema.get("required").is_none());
}

fn format_path(path: &str, bytes: &[u8], config: &Configuration) -> FormatResult {
    TombiPluginHandler::default().format(
        SyncFormatRequest {
            file_path: Path::new(path),
            file_bytes: bytes.to_vec(),
            config_id: FormatConfigId::uninitialized(),
            config,
            range: None,
            token: &NullCancellationToken,
        },
        |_| panic!("TOML formatting must not invoke another plugin"),
    )
}

#[test]
fn a_built_in_schema_decides_like_in_the_cli() {
    // Tombi's own schema for tombi.toml says TOML v1.1.0, which keeps a
    // multi-line inline table with a trailing comma, as `tombi format` does
    let multi_line = b"catalog = {\n  paths = [\n    \"a\",\n    \"b\",\n  ],\n}\n";
    let config = Configuration::default();
    assert!(
        format_path("tombi.toml", multi_line, &config)
            .unwrap()
            .is_none()
    );
    // any other file goes by the configured TOML version, v1.0.0, without
    // multi-line inline tables
    assert_eq!(
        format_path("other.toml", multi_line, &config).unwrap(),
        Some(b"catalog = { paths = [\n  \"a\",\n  \"b\",\n] }\n".to_vec())
    );
}

#[test]
fn schemas_with_unknown_properties_are_diagnosed() {
    let config = resolve(
        json!({ "schemas": [{ "include": ["*.toml"] }] }),
        &GlobalConfiguration::default(),
    );
    assert_eq!(config.diagnostics.len(), 1);
    assert_eq!(config.diagnostics[0].property_name, "schemas");
}

#[cfg(feature = "schema")]
#[test]
fn the_schema_names_the_options_of_schemas_like_the_configuration() {
    let schema = dprint_plugin_tombi::configuration_schema().unwrap();
    let root = schema["$defs"]["RootSchema"]["properties"]
        .as_object()
        .unwrap();
    assert!(root.contains_key("tomlVersion"), "{root:?}");
    // and nested options, ex. a schema's format rules (`tableKeysOrder`)
    for (name, definition) in schema["$defs"].as_object().unwrap() {
        if let Some(properties) = definition["properties"].as_object() {
            assert!(
                properties.keys().all(|key| !key.contains('-')),
                "{name}: {properties:?}"
            );
        }
    }
}
