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
use tombi_formatter::Formatter;
use tombi_schema_store::{Options, SchemaStore};

use crate::{Configuration, SCHEMA_URL, UPDATE_URL, configuration};

/// Stateless adapter. Each request gets its own offline schema store.
#[derive(Default)]
pub struct TombiPluginHandler;

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
        let store = SchemaStore::new_with_options(Options {
            offline: Some(true),
            cache: None,
            ..Options::default()
        });
        let formatter = Formatter::new(
            request.config.toml_version,
            &options,
            Some(Either::Right(request.file_path)),
            &store,
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
