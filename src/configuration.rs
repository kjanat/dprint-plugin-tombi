use dprint_core::configuration::{
    ConfigKeyMap, ConfigurationDiagnostic, GlobalConfiguration, get_unknown_property_diagnostics,
};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value, json};
use tombi_config::{SchemaItem, TomlVersion, format::FormatRules};

const TOML_VERSION_ALIASES: [(&str, &str); 2] = [("1.0.0", "v1.0.0"), ("1.1.0", "v1.1.0")];

/// Validated Tombi options. Unset rules retain upstream defaults.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    /// The TOML language version. Document directives may override it.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub toml_version: TomlVersion,
    /// All upstream formatting rules, exposed as camelCase dprint keys.
    #[serde(flatten, serialize_with = "serialize_rules")]
    #[cfg_attr(feature = "schema", schemars(with = "FormatRules"))]
    pub rules: FormatRules,
    /// Tombi's `[[schemas]]`, with camelCase keys (ex. `tomlVersion`): the
    /// schemas of the files they include, ahead of the ones built into Tombi.
    /// A schema can give its files a TOML version, ex. `v1.0.0` for
    /// `Cargo.toml`, which Cargo reads as TOML 1.0. Remote schemas aren't
    /// fetched, so only what an entry says itself applies to its files.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub schemas: Vec<SchemaItem>,
}

fn camel_case(key: &str) -> String {
    let mut result = String::with_capacity(key.len());
    let mut uppercase = false;
    for character in key.chars() {
        if character == '-' {
            uppercase = true;
        } else {
            result.push(if uppercase {
                character.to_ascii_uppercase()
            } else {
                character
            });
            uppercase = false;
        }
    }
    result
}

fn serialize_rules<S: Serializer>(rules: &FormatRules, serializer: S) -> Result<S::Ok, S::Error> {
    use serde::ser::Error;
    let value = serde_json::to_value(rules).map_err(S::Error::custom)?;
    let Value::Object(properties) = value else {
        return Err(S::Error::custom(
            "Tombi format rules must serialize as an object",
        ));
    };
    let properties: Map<String, Value> = properties
        .into_iter()
        .map(|(key, value)| (camel_case(&key), value))
        .collect();
    properties.serialize(serializer)
}

pub fn resolve_config(
    mut config: ConfigKeyMap,
    global: &GlobalConfiguration,
) -> (Configuration, Vec<ConfigurationDiagnostic>) {
    let mut diagnostics = Vec::new();
    let toml_version = resolve_version(&mut config, &mut diagnostics);
    let schemas = resolve_schemas(&mut config, &mut diagnostics);
    let inherited = inherited_rules(global);
    // These Option fields serialize as null. Discovering their names from the
    // upstream type avoids a second, inevitably stale list of supported rules.
    let Value::Object(defaults) = serde_json::to_value(FormatRules::default())
        .expect("default Tombi rules contain only null fields")
    else {
        unreachable!("Tombi rules are an object");
    };
    let mut rules = Map::new();
    for upstream_key in defaults.keys() {
        let key = camel_case(upstream_key);
        let value = config.shift_remove(&key).map_or_else(
            || inherited.get(upstream_key).cloned(),
            |value| Some(json!(value)),
        );
        let Some(value) = value else { continue };
        let candidate = Value::Object(Map::from_iter([(upstream_key.clone(), value.clone())]));
        match serde_json::from_value::<FormatRules>(candidate) {
            Ok(_) => {
                rules.insert(upstream_key.clone(), value);
            }
            Err(error) => diagnostics.push(ConfigurationDiagnostic {
                property_name: key,
                message: error.to_string(),
            }),
        }
    }
    diagnostics.extend(get_unknown_property_diagnostics(config));
    (
        Configuration {
            toml_version,
            // Each field was deserialized independently above. There are no
            // cross-field deserialization constraints in the upstream type.
            rules: serde_json::from_value(Value::Object(rules))
                .expect("individually validated Tombi rules must deserialize together"),
            schemas,
        },
        diagnostics,
    )
}

fn inherited_rules(global: &GlobalConfiguration) -> Map<String, Value> {
    let mut rules = Map::new();
    if let Some(width) = global.line_width {
        rules.insert("line-width".into(), json!(width));
    }
    if let Some(width) = global.indent_width {
        rules.insert("indent-width".into(), json!(width));
    }
    if let Some(tabs) = global.use_tabs {
        rules.insert(
            "indent-style".into(),
            json!(if tabs { "tab" } else { "space" }),
        );
    }
    if let Some(ending) = global.new_line_kind {
        rules.insert("line-ending".into(), json!(ending));
    }
    rules
}

/// The `[[schemas]]` of the configuration, from camelCase keys to Tombi's
/// kebab-case ones.
fn resolve_schemas(
    config: &mut ConfigKeyMap,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> Vec<SchemaItem> {
    let Some(value) = config.shift_remove("schemas") else {
        return Vec::new();
    };
    serde_json::from_value(kebab_case_keys(json!(value))).unwrap_or_else(|error| {
        diagnostics.push(ConfigurationDiagnostic {
            property_name: "schemas".into(),
            message: error.to_string(),
        });
        Vec::new()
    })
}

/// The value with the keys of its objects, at any depth, in kebab case.
fn kebab_case_keys(value: Value) -> Value {
    match value {
        Value::Object(properties) => Value::Object(
            properties
                .into_iter()
                .map(|(key, value)| (kebab_case(&key), kebab_case_keys(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(kebab_case_keys).collect()),
        value => value,
    }
}

fn kebab_case(key: &str) -> String {
    let mut result = String::with_capacity(key.len() + 4);
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            result.push('-');
            result.push(character.to_ascii_lowercase());
        } else {
            result.push(character);
        }
    }
    result
}

fn resolve_version(
    config: &mut ConfigKeyMap,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> TomlVersion {
    let Some(value) = config.shift_remove("tomlVersion") else {
        return TomlVersion::default();
    };
    let mut value = json!(value);
    // Preserve the unprefixed spellings accepted by the original scaffold.
    if let Some((_, canonical)) = TOML_VERSION_ALIASES
        .iter()
        .find(|(alias, _)| value.as_str() == Some(*alias))
    {
        value = json!(canonical);
    }
    serde_json::from_value(value).unwrap_or_else(|error| {
        diagnostics.push(ConfigurationDiagnostic {
            property_name: "tomlVersion".into(),
            message: error.to_string(),
        });
        TomlVersion::default()
    })
}

/// Names the properties of the definitions `schemas` refers to, at any depth,
/// in camelCase, as `resolve_schemas` reads them (ex. `tomlVersion` for
/// Tombi's `toml-version`). Only `schemas` refers to definitions with
/// properties; the formatting rules are top-level properties.
#[cfg(feature = "schema")]
fn camel_case_schema_definitions(schema: &mut Value) {
    fn references(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::Object(properties) => {
                if let Some(Value::String(reference)) = properties.get("$ref")
                    && let Some(name) = reference.strip_prefix("#/$defs/")
                {
                    found.push(name.to_string());
                }
                properties
                    .values()
                    .for_each(|value| references(value, found));
            }
            Value::Array(values) => values.iter().for_each(|value| references(value, found)),
            _ => {}
        }
    }

    let mut pending = Vec::new();
    references(&schema["properties"]["schemas"], &mut pending);
    let mut renamed = std::collections::BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !renamed.insert(name.clone()) {
            continue;
        }
        let Some(definition) = schema["$defs"].get_mut(&name) else {
            continue;
        };
        references(definition, &mut pending);
        if let Some(Value::Object(properties)) = definition.get_mut("properties") {
            *properties = std::mem::take(properties)
                .into_iter()
                .map(|(key, value)| (camel_case(&key), value))
                .collect();
        }
        if let Some(Value::Array(required)) = definition.get_mut("required") {
            for name in required.iter_mut() {
                if let Value::String(key) = name {
                    *key = camel_case(key);
                }
            }
        }
    }
}

/// Generate the schema from the same upstream Rust types used for validation.
///
/// # Errors
/// Returns an error if the derived schema does not have an object shape.
#[cfg(feature = "schema")]
pub fn configuration_schema() -> anyhow::Result<Value> {
    let mut schema = serde_json::to_value(schemars::schema_for!(Configuration))?;
    let properties = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow::anyhow!("configuration schema has no properties"))?;
    *properties = std::mem::take(properties)
        .into_iter()
        .map(|(key, value)| (camel_case(&key), value))
        .collect();
    let version = properties
        .get_mut("tomlVersion")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow::anyhow!("configuration schema has no TOML version"))?;
    let reference = version
        .remove("$ref")
        .ok_or_else(|| anyhow::anyhow!("TOML version schema has no reference"))?;
    version.insert(
        "anyOf".into(),
        json!([
            {"$ref": reference},
            {"type": "string", "enum": TOML_VERSION_ALIASES.map(|(alias, _)| alias)}
        ]),
    );
    // TOML version has a runtime default, so it is not a required config key.
    schema
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("schema is not an object"))?
        .remove("required");
    camel_case_schema_definitions(&mut schema);
    schema["$id"] = json!(crate::SCHEMA_URL);
    schema["title"] = json!("dprint-plugin-tombi Configuration");
    schema["additionalProperties"] = json!(false);
    Ok(schema)
}
