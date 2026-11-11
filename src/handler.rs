use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use dprint_core::configuration::{ConfigKeyMap, GlobalConfiguration};
use dprint_core::plugins::{
    CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatConfigId, FormatError,
    FormatResult, PluginInfo, PluginResolveConfigurationResult, SyncFormatRequest,
    SyncHostFormatRequest, SyncPluginHandler,
};
use itertools::Either;
use tombi_config::SchemaItem;
use tombi_formatter::Formatter;
use tombi_schema_store::{Options, SchemaStore};

use crate::{Configuration, SCHEMA_URL, UPDATE_URL, configuration};

/// Formats TOML with Tombi's built-in schemas and schemas supplied by kprint.
#[derive(Default)]
pub struct TombiPluginHandler {
    store: Option<(FormatConfigId, SchemaStore)>,
}

impl TombiPluginHandler {
    /// Creates a formatter with no cached schemas.
    #[must_use]
    pub const fn new() -> Self {
        Self { store: None }
    }

    fn store(
        &mut self,
        id: FormatConfigId,
        config: &Configuration,
    ) -> Result<&SchemaStore, FormatError> {
        if !self
            .store
            .as_ref()
            .is_some_and(|(cached_id, _)| *cached_id == id)
        {
            self.store = None;
            self.store = Some((id, schema_store(config)?));
        }
        Ok(&self.store.as_ref().expect("schema store was initialized").1)
    }
}

/// Loads schemas into Tombi's in-memory filesystem before resolving references.
fn schema_store(settings: &Configuration) -> Result<SchemaStore, FormatError> {
    tombi_fs::clear();
    for (path, text) in &settings.schema_context.files {
        if !path.starts_with("https://") && !path.starts_with("http://") {
            tombi_fs::set_file(formatter_path(std::path::Path::new(path)), text);
        }
    }
    let schema = settings.schema_context.options.clone().unwrap_or_default();
    let store = SchemaStore::new_with_options_and_http_client(
        Options {
            offline: Some(false),
            cache: None,
            strict: schema.strict,
            format: schema.format.clone(),
            lint: schema.lint.clone(),
            ..Options::default()
        },
        crate::resources::schema_client(
            settings.schema_context.files.clone(),
            &settings.schema_context.file_sources,
        ),
    );
    // only the built-in catalog: the default also lists JSON Schema Store's,
    // which would have to be fetched
    let mut schema =
        serde_json::to_value(schema).map_err(|err| FormatError::new(err.to_string()))?;
    schema["catalog"] =
        serde_json::json!({ "paths": [tombi_config::TOMBI_SCHEMASTORE_CATALOG_URL] });
    let config: tombi_config::Config = serde_json::from_value(serde_json::json!({
        "schema": schema,
        "schemas": with_built_in_copies(&settings.schemas, &settings.schema_context.files)?,
    }))
    .map_err(|err| FormatError::new(format!("Configuring Tombi's built-in schemas: {err}")))?;
    let path = settings.schema_context.path.as_deref().map(formatter_path);
    ready(store.load_config(&config, path.as_deref()))?
        .map_err(|err| FormatError::new(format!("Loading Tombi's built-in schemas: {err}")))?;
    Ok(store)
}

impl SyncPluginHandler<Configuration> for TombiPluginHandler {
    fn plugin_info(&mut self) -> PluginInfo {
        PluginInfo {
            name: env!("CARGO_PKG_NAME").into(),
            version: env!("CARGO_PKG_VERSION").into(),
            config_key: "toml".into(),
            help_url: env!("CARGO_PKG_REPOSITORY").into(),
            config_schema_url: SCHEMA_URL.into(),
            update_url: Some(UPDATE_URL.into()),
            supports_range_formatting: false,
        }
    }

    fn license_text(&mut self) -> String {
        concat!(
            include_str!("../LICENSE"),
            "\nTombi:\n",
            include_str!("../LICENSE-TOMBI")
        )
        .into()
    }

    fn resolve_config(
        &mut self,
        config: ConfigKeyMap,
        global_config: &GlobalConfiguration,
    ) -> PluginResolveConfigurationResult<Configuration> {
        self.store = None;
        let (config, diagnostics) = configuration::resolve_config(config, global_config);
        PluginResolveConfigurationResult {
            config,
            diagnostics,
            file_matching: FileMatchingInfo {
                file_extensions: vec!["toml".into()],
                file_names: vec![],
                additive: false,
            },
        }
    }

    fn check_config_updates(
        &self,
        _message: CheckConfigUpdatesMessage,
    ) -> Result<Vec<ConfigChange>, FormatError> {
        Ok(vec![])
    }

    fn format(
        &mut self,
        request: SyncFormatRequest<Configuration>,
        _format_with_host: impl FnMut(SyncHostFormatRequest) -> FormatResult,
    ) -> FormatResult {
        if request.token.is_cancelled() {
            return Ok(None);
        }
        if request.range.is_some() {
            return Err(FormatError::new("Tombi does not support range formatting"));
        }
        let source = std::str::from_utf8(&request.file_bytes)?;
        let options = tombi_config::FormatOptions {
            rules: Some(request.config.rules.clone()),
        };
        let store = self.store(request.config_id, request.config)?;
        let path = formatter_path(request.file_path);
        let formatter = Formatter::new(
            request.config.toml_version,
            &options,
            Some(Either::Right(&path)),
            store,
        );
        let output = ready(formatter.format(source))?.map_err(|diagnostics| {
            FormatError::new(
                diagnostics
                    .iter()
                    .map(|diagnostic| format!("[{}] {}", diagnostic.code(), diagnostic.message()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })?;
        if request.token.is_cancelled() || output == source {
            Ok(None)
        } else {
            Ok(Some(output.into_bytes()))
        }
    }
}

pub fn formatter_path(path: &std::path::Path) -> std::path::PathBuf {
    #[cfg(target_family = "wasm")]
    {
        let path = path.to_string_lossy().replace('\\', "/");
        if path.as_bytes().get(1) == Some(&b':') {
            return std::path::PathBuf::from(format!("/{path}"));
        }
        std::path::PathBuf::from(path)
    }
    #[cfg(not(target_family = "wasm"))]
    path.to_path_buf()
}

/// The schemas, with a remote schema that Tombi has built in (ex. JSON Schema
/// Store's `cargo.json`) taken from its built-in copy. The sandbox can't fetch
/// it, and a schema that doesn't load doesn't apply what its entry says (ex.
/// `toml-version`), while the CLI has it cached from an earlier run.
fn with_built_in_copies(
    schemas: &[SchemaItem],
    files: &std::collections::BTreeMap<String, String>,
) -> Result<serde_json::Value, FormatError> {
    let mut schemas = serde_json::to_value(schemas)
        .map_err(|err| FormatError::new(format!("Reading the configured schemas: {err}")))?;
    let entries = schemas.as_array_mut().into_iter().flatten();
    for path in entries.filter_map(|entry| entry.get_mut("path")) {
        let built_in = path
            .as_str()
            .filter(|remote| !files.contains_key(*remote))
            .and_then(|remote| remote.strip_prefix("https://"))
            .map(|rest| format!("tombi://{rest}"))
            .filter(|built_in| {
                built_in.parse::<tombi_uri::Uri>().is_ok_and(|uri| {
                    tombi_schema_store::get_tombi_schemastore_content(&uri).is_some()
                })
            });
        if let Some(built_in) = built_in {
            *path = serde_json::Value::String(built_in);
        }
    }
    Ok(schemas)
}

// With no HTTP backend, no disk cache and an uncontended per-request store,
// this path must complete synchronously. Never spin or block a Wasm host if
// an upstream change introduces a future that needs an external executor.
pub fn ready<F: Future>(future: F) -> Result<F::Output, FormatError> {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => Ok(result),
        Poll::Pending => Err(FormatError::new(
            "Tombi requested asynchronous I/O unavailable in dprint's Wasm sandbox",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::ready;

    #[test]
    fn pending_future_is_an_error_not_a_busy_loop() {
        assert!(ready(std::future::pending::<()>()).is_err());
    }

    #[test]
    fn ready_future_needs_no_executor() {
        assert_eq!(ready(std::future::ready(42)).unwrap(), 42);
    }
}
