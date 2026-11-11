use dprint_core::configuration::{
    ConfigKeyMap, ConfigurationDiagnostic, GlobalConfiguration, get_unknown_property_diagnostics,
};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value, json};
use tombi_config::{SchemaItem, TomlVersion, format::FormatRules};

const TOML_VERSION_ALIASES: [(&str, &str); 2] = [("1.0.0", "v1.0.0"), ("1.1.0", "v1.1.0")];

/// Kprint searches these files in order. It uses `pyproject.toml` only if
/// the file contains a `[tool.tombi]` table.
pub const CONFIG_FILES: [&str; 4] = [
    ".tombi.toml",
    "tombi.toml",
    ".config/tombi.toml",
    "pyproject.toml#tool.tombi",
];
/// The user's configuration-file selection, consumed by kprint.
const CONFIG_FILE_PROPERTY: &str = "configFile";
/// Configuration text, parsed values and downloaded files supplied by kprint.
const CONFIG_FILE_CONTENT_PROPERTY: &str = "$configFile";

/// Settings read from Tombi's configuration file before applying plugin options.
#[derive(Default)]
struct TombiFile {
    path: String,
    absolute_path: Option<std::path::PathBuf>,
    files: std::collections::BTreeMap<String, String>,
    file_sources: std::collections::BTreeMap<String, String>,
    schema: Option<tombi_config::SchemaOverviewOptions>,
    toml_version: Option<Value>,
    rules: Map<String, Value>,
    schemas: Option<Value>,
}

impl TombiFile {
    fn take(config: &mut ConfigKeyMap, diagnostics: &mut Vec<ConfigurationDiagnostic>) -> Self {
        if config.shift_remove(CONFIG_FILE_PROPERTY).is_some() {
            diagnostics.push(ConfigurationDiagnostic {
                property_name: CONFIG_FILE_PROPERTY.into(),
                message: "This dprint doesn't pass plugins a configuration file of their own; \
                          configFile needs one that does (kjanat/dprint). Set the plugin's own \
                          properties instead."
                    .into(),
            });
        }
        let Some(value) = config.shift_remove(CONFIG_FILE_CONTENT_PROPERTY) else {
            return Self::default();
        };
        let value = json!(value);
        let path = value["path"]
            .as_str()
            .unwrap_or("the configuration file")
            .to_string();
        let mut content = match value["content"].as_str().map(serde_json::from_str::<Value>) {
            Some(Ok(content)) => content,
            Some(Err(error)) => {
                diagnostics.push(ConfigurationDiagnostic {
                    property_name: CONFIG_FILE_CONTENT_PROPERTY.into(),
                    message: format!("{path}: {error}"),
                });
                return Self::default();
            }
            None => {
                diagnostics.push(ConfigurationDiagnostic {
                    property_name: CONFIG_FILE_CONTENT_PROPERTY.into(),
                    message: format!("{path}: dprint passed no content"),
                });
                return Self::default();
            }
        };
        if value["text"].is_string()
            && (content.get("extends").is_some() || value["absolutePath"].is_null())
        {
            match resolve_inheritance(&value, &content) {
                Ok(resolved) => content = resolved,
                Err(error) => diagnostics.push(ConfigurationDiagnostic {
                    property_name: CONFIG_FILE_CONTENT_PROPERTY.into(),
                    message: format!("{path}: {error}"),
                }),
            }
        }
        let files = supplied_files(&value, "files", &path, diagnostics);
        let schema = content
            .get("schema")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .unwrap_or_else(|error| {
                diagnostics.push(ConfigurationDiagnostic {
                    property_name: format!("{path}: schema"),
                    message: error.to_string(),
                });
                None
            });
        let file_sources = supplied_files(&value, "fileSources", &path, diagnostics);
        Self {
            absolute_path: value["absolutePath"]
                .as_str()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    value["baseDirectory"]
                        .as_str()
                        .map(|base| std::path::Path::new(base).join("tombi.toml"))
                }),
            files,
            file_sources,
            schema,
            toml_version: content
                .get("toml-version")
                .filter(|value| !value.is_null())
                .cloned(),
            rules: content
                .pointer("/format/rules")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            schemas: content
                .get("schemas")
                .filter(|value| !value.is_null())
                .cloned(),
            path,
        }
    }
}

fn supplied_files(
    value: &Value,
    key: &str,
    path: &str,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> std::collections::BTreeMap<String, String> {
    value
        .get(key)
        .map_or_else(
            || Ok(std::collections::BTreeMap::new()),
            |files| serde_json::from_value(files.clone()),
        )
        .unwrap_or_else(|error| {
            diagnostics.push(ConfigurationDiagnostic {
                property_name: CONFIG_FILE_CONTENT_PROPERTY.into(),
                message: format!("{path}: invalid {key}: {error}"),
            });
            std::collections::BTreeMap::new()
        })
}

fn resolve_urls(text: &str, source: &str) -> anyhow::Result<String> {
    let store = tombi_schema_store::SchemaStore::new_with_options(tombi_schema_store::Options {
        offline: Some(true),
        ..Default::default()
    });
    let parser = serde_tombi::Deserializer::builder()
        .schema_store(&store)
        .build();
    let mut content: Value = crate::handler::ready(parser.from_str_async(text))??;
    serialize_remote_config(&mut content, source)
}

fn serialize_remote_config(content: &mut Value, source: &str) -> anyhow::Result<String> {
    let store = tombi_schema_store::SchemaStore::new_with_options(tombi_schema_store::Options {
        offline: Some(true),
        ..Default::default()
    });
    let base = source.parse::<tombi_uri::Uri>()?;
    if let Some(extends) = content.get_mut("extends").and_then(Value::as_array_mut) {
        for path in extends {
            if let Some(relative) = path.as_str() {
                *path = Value::String(base.join(relative)?.to_string());
            }
        }
    }
    if let Some(schemas) = content.get_mut("schemas").and_then(Value::as_array_mut) {
        for schema in schemas {
            if let Some(path) = schema.get_mut("path")
                && let Some(relative) = path.as_str()
            {
                *path = Value::String(base.join(relative)?.to_string());
            }
        }
    }
    let serializer = serde_tombi::Serializer::builder()
        .schema_store(&store)
        .build();
    Ok(crate::handler::ready(
        serializer.to_string_async(&content),
    )??)
}

fn resolve_inheritance(value: &Value, selected: &Value) -> anyhow::Result<Value> {
    #[derive(serde::Deserialize)]
    struct Document {
        source: String,
        text: String,
    }
    let documents: std::collections::BTreeMap<String, Document> = value
        .get("documents")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?
        .unwrap_or_default();
    let source = value["source"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("kprint supplied no configuration source"))?;
    let remote = source.starts_with("https://") || source.starts_with("http://");
    let mut documents = documents
        .into_iter()
        .map(|(uri, document)| {
            let text = if document.source.starts_with("https://")
                || document.source.starts_with("http://")
            {
                resolve_urls(&document.text, &document.source)?
            } else {
                document.text
            };
            Ok((uri, text))
        })
        .collect::<anyhow::Result<std::collections::BTreeMap<_, _>>>()?;
    let root_text = if remote {
        serialize_remote_config(&mut selected.clone(), source)?
    } else {
        let store =
            tombi_schema_store::SchemaStore::new_with_options(tombi_schema_store::Options {
                offline: Some(true),
                ..Default::default()
            });
        let serializer = serde_tombi::Serializer::builder()
            .schema_store(&store)
            .build();
        let selected = if value["absolutePath"].as_str().is_some_and(|path| {
            crate::handler::formatter_path(std::path::Path::new(path))
                .file_name()
                .is_some_and(|name| name == "pyproject.toml")
        }) {
            json!({"tool": {"tombi": selected}})
        } else {
            selected.clone()
        };
        crate::handler::ready(serializer.to_string_async(&selected))??
    };
    documents.insert(source.to_owned(), root_text.clone());
    tombi_fs::clear();
    for (uri, text) in &documents {
        if let Ok(uri) = uri.parse::<tombi_uri::Uri>()
            && let Ok(path) = uri.to_file_path()
        {
            tombi_fs::set_file(crate::handler::formatter_path(&path), text);
        }
    }
    let http = tombi_schema_store::CachedHttpClient::new(
        crate::resources::client(documents),
        tombi_schema_store::HttpClientOptions {
            offline: false,
            cache: None,
        },
    );
    let path = value["absolutePath"].as_str().map_or_else(
        || std::path::PathBuf::from("/tombi.toml"),
        std::path::PathBuf::from,
    );
    let config = crate::handler::ready(serde_tombi::config::from_str(
        &root_text,
        &crate::handler::formatter_path(&path),
        &http,
    ))??;
    let mut config = serde_json::to_value(config)?;
    omit_unset_options(&mut config);
    Ok(config)
}

// Tombi serializes absent options as null. Remove them so they do not reset
// formatting options inherited from kprint's global configuration.
fn omit_unset_options(value: &mut Value) {
    match value {
        Value::Object(properties) => {
            properties.retain(|_, value| !value.is_null());
            for value in properties.values_mut() {
                omit_unset_options(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                omit_unset_options(value);
            }
        }
        _ => {}
    }
}

/// Validated Tombi options. Unset rules retain upstream defaults.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    #[serde(skip)]
    pub(crate) schema_context: SchemaContext,
    /// The TOML language version. Document directives may override it.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub toml_version: TomlVersion,
    /// All upstream formatting rules, exposed as camelCase dprint keys.
    #[serde(flatten, serialize_with = "serialize_rules")]
    #[cfg_attr(feature = "schema", schemars(with = "FormatRules"))]
    pub rules: FormatRules,
    /// Tombi's `[[schemas]]`, with camelCase keys such as `tomlVersion`.
    /// These associations take precedence over Tombi's built-in catalog.
    /// Kprint supplies local and downloaded schemas. If none was supplied,
    /// the plugin uses Tombi's built-in copy when available.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub schemas: Vec<SchemaItem>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchemaContext {
    pub path: Option<std::path::PathBuf>,
    pub files: std::collections::BTreeMap<String, String>,
    pub file_sources: std::collections::BTreeMap<String, String>,
    pub options: Option<tombi_config::SchemaOverviewOptions>,
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
    let file = TombiFile::take(&mut config, &mut diagnostics);
    let toml_version = resolve_version(&mut config, &file, &mut diagnostics);
    let schemas = resolve_schemas(&mut config, &file, &mut diagnostics);
    if file
        .schema
        .as_ref()
        .is_none_or(|schema| schema.enabled.unwrap_or_default().value())
        && file.absolute_path.is_none()
        && schemas.iter().any(|schema| {
            schema
                .path()
                .parse::<tombi_uri::Uri>()
                .map_or(true, |uri| uri.scheme() == "file")
        })
    {
        diagnostics.push(ConfigurationDiagnostic {
            property_name: "schemas".into(),
            message:
                "Local schemas require a kprint version that supplies schema files to plugins."
                    .into(),
        });
    }
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
        // the plugin's own property, then Tombi's configuration file, then
        // dprint's global configuration
        let (value, property_name) = match config.shift_remove(&key) {
            Some(value) => (json!(value), key),
            None => match file.rules.get(upstream_key) {
                Some(value) => (
                    value.clone(),
                    format!("{}: format.rules.{upstream_key}", file.path),
                ),
                None => match inherited.get(upstream_key) {
                    Some(value) => (value.clone(), key),
                    None => continue,
                },
            },
        };
        let candidate = Value::Object(Map::from_iter([(upstream_key.clone(), value.clone())]));
        match serde_json::from_value::<FormatRules>(candidate) {
            Ok(_) => {
                rules.insert(upstream_key.clone(), value);
            }
            Err(error) => diagnostics.push(ConfigurationDiagnostic {
                property_name,
                message: error.to_string(),
            }),
        }
    }
    diagnostics.extend(get_unknown_property_diagnostics(config));
    (
        Configuration {
            schema_context: SchemaContext {
                path: file.absolute_path,
                file_sources: file.file_sources,
                files: file.files,
                options: file.schema,
            },
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
/// Or the configuration file's, which has Tombi's keys already.
fn resolve_schemas(
    config: &mut ConfigKeyMap,
    file: &TombiFile,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> Vec<SchemaItem> {
    let (value, property_name) = match config.shift_remove("schemas") {
        Some(value) => (kebab_case_keys(json!(value)), "schemas".to_string()),
        None => match &file.schemas {
            Some(value) => (value.clone(), format!("{}: schemas", file.path)),
            None => return Vec::new(),
        },
    };
    serde_json::from_value(value).unwrap_or_else(|error| {
        diagnostics.push(ConfigurationDiagnostic {
            property_name,
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
    file: &TombiFile,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> TomlVersion {
    let (mut value, property_name) = match config.shift_remove("tomlVersion") {
        Some(value) => (json!(value), "tomlVersion".to_string()),
        None => match &file.toml_version {
            Some(value) => (value.clone(), format!("{}: toml-version", file.path)),
            None => return TomlVersion::default(),
        },
    };
    // Preserve the unprefixed spellings accepted by the original scaffold.
    if let Some((_, canonical)) = TOML_VERSION_ALIASES
        .iter()
        .find(|(alias, _)| value.as_str() == Some(*alias))
    {
        value = json!(canonical);
    }
    serde_json::from_value(value).unwrap_or_else(|error| {
        diagnostics.push(ConfigurationDiagnostic {
            property_name,
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
