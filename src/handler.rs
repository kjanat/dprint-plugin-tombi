use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use dprint_core::configuration::{ConfigKeyMap, GlobalConfiguration};
use dprint_core::plugins::{
    CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatError, FormatResult,
    PluginInfo, PluginResolveConfigurationResult, SyncFormatRequest, SyncHostFormatRequest,
    SyncPluginHandler,
};
use itertools::Either;
use tombi_config::SchemaItem;
use tombi_formatter::Formatter;
use tombi_schema_store::{Options, SchemaStore};

use crate::{Configuration, SCHEMA_URL, UPDATE_URL, configuration};

/// The adapter between dprint and Tombi's formatter.
///
/// It formats with an offline schema store that knows the configured
/// `schemas` and the schemas built into Tombi (ex. for `Cargo.toml`,
/// `pyproject.toml` and `tombi.toml`), as the CLI does, and nothing it would
/// have to fetch. A schema can decide how a file formats: `tombi.toml`'s says
/// TOML v1.1.0, which keeps multi-line inline tables. A store is made for the
/// first file with its `schemas` and kept, as it's the same for every file.
#[derive(Default)]
pub struct TombiPluginHandler {
    stores: Vec<(Vec<SchemaItem>, SchemaStore)>,
}

impl TombiPluginHandler {
    /// A handler without stores yet, which the plugin's static holds.
    #[must_use]
    pub const fn new() -> Self {
        Self { stores: Vec::new() }
    }

    fn store(&mut self, schemas: &[SchemaItem]) -> Result<&SchemaStore, FormatError> {
        if let Some(index) = self.stores.iter().position(|(stored, _)| stored == schemas) {
            return Ok(&self.stores[index].1);
        }
        self.stores.push((schemas.to_vec(), schema_store(schemas)?));
        Ok(&self.stores[self.stores.len() - 1].1)
    }
}

/// An offline store with the schemas and Tombi's built-in catalog, which is
/// part of the plugin, so loading it reads nothing from outside the sandbox.
fn schema_store(schemas: &[SchemaItem]) -> Result<SchemaStore, FormatError> {
    let store = SchemaStore::new_with_options(Options {
        offline: Some(true),
        cache: None,
        ..Options::default()
    });
    // only the built-in catalog: the default also lists JSON Schema Store's,
    // which would have to be fetched
    let config: tombi_config::Config = serde_json::from_value(serde_json::json!({
        "schema": { "catalog": { "paths": [tombi_config::TOMBI_SCHEMASTORE_CATALOG_URL] } },
        "schemas": with_built_in_copies(schemas)?,
    }))
    .map_err(|err| FormatError::new(format!("Configuring Tombi's built-in schemas: {err}")))?;
    ready(store.load_config(&config, None))?
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
        let (config, diagnostics) = configuration::resolve_config(config, global_config);
        PluginResolveConfigurationResult {
            config,
            diagnostics,
            file_matching: FileMatchingInfo {
                file_extensions: vec!["toml".into()],
                file_names: vec![],
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
        let store = self.store(&request.config.schemas)?;
        let formatter = Formatter::new(
            request.config.toml_version,
            &options,
            Some(Either::Right(request.file_path)),
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

/// The schemas, with a remote schema that Tombi has built in (ex. JSON Schema
/// Store's `cargo.json`) taken from its built-in copy. The sandbox can't fetch
/// it, and a schema that doesn't load doesn't apply what its entry says (ex.
/// `toml-version`), while the CLI has it cached from an earlier run.
fn with_built_in_copies(schemas: &[SchemaItem]) -> Result<serde_json::Value, FormatError> {
    let mut schemas = serde_json::to_value(schemas)
        .map_err(|err| FormatError::new(format!("Reading the configured schemas: {err}")))?;
    let entries = schemas.as_array_mut().into_iter().flatten();
    for path in entries.filter_map(|entry| entry.get_mut("path")) {
        let built_in = path
            .as_str()
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
fn ready<F: Future>(future: F) -> Result<F::Output, FormatError> {
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
