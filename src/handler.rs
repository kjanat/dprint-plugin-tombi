use anyhow::Result;
use dprint_core::configuration::{
    ConfigKeyMap, GlobalConfiguration, get_unknown_property_diagnostics, get_value,
};
use dprint_core::plugins::{
    CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatResult, PluginInfo,
    PluginResolveConfigurationResult, SyncFormatRequest, SyncHostFormatRequest, SyncPluginHandler,
};

use crate::configuration::{
    Configuration, DateTimeDelimiterOption, IndentStyleOption, LineEndingOption,
    StringQuoteStyleOption, TomlVersionOption,
};

#[derive(Default)]
pub struct TombiPluginHandler;

impl SyncPluginHandler<Configuration> for TombiPluginHandler {
    fn plugin_info(&mut self) -> PluginInfo {
        PluginInfo {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_key: "toml".to_string(),
            help_url: "https://github.com/tombi-toml/dprint-plugin-tombi".to_string(),
            config_schema_url: String::new(),
            update_url: None,
        }
    }

    fn license_text(&mut self) -> String {
        include_str!("../LICENSE").to_string()
    }

    fn resolve_config(
        &mut self,
        config: ConfigKeyMap,
        global_config: &GlobalConfiguration,
    ) -> PluginResolveConfigurationResult<Configuration> {
        let mut config = config;
        let mut diagnostics = Vec::new();

        // Parse string values into enums
        let indent_style_str: String = get_value(
            &mut config,
            "indentStyle",
            match global_config.use_tabs {
                Some(true) => "tab".to_string(),
                _ => "space".to_string(),
            },
            &mut diagnostics,
        );
        let indent_style = match indent_style_str.as_str() {
            "tab" => IndentStyleOption::Tab,
            _ => IndentStyleOption::Space,
        };

        let line_ending_str: String = get_value(
            &mut config,
            "lineEnding",
            match &global_config.new_line_kind {
                Some(kind) => match kind {
                    dprint_core::configuration::NewLineKind::LineFeed => "lf".to_string(),
                    dprint_core::configuration::NewLineKind::CarriageReturnLineFeed => {
                        "crlf".to_string()
                    }
                    _ => "lf".to_string(),
                },
                None => "lf".to_string(),
            },
            &mut diagnostics,
        );
        let line_ending = match line_ending_str.as_str() {
            "crlf" => LineEndingOption::Crlf,
            _ => LineEndingOption::Lf,
        };

        let string_quote_style_str: String = get_value(
            &mut config,
            "stringQuoteStyle",
            "double".to_string(),
            &mut diagnostics,
        );
        let string_quote_style = match string_quote_style_str.as_str() {
            "single" => StringQuoteStyleOption::Single,
            "preserve" => StringQuoteStyleOption::Preserve,
            _ => StringQuoteStyleOption::Double,
        };

        let date_time_delimiter_str: String = get_value(
            &mut config,
            "dateTimeDelimiter",
            "T".to_string(),
            &mut diagnostics,
        );
        let date_time_delimiter = match date_time_delimiter_str.as_str() {
            "space" => DateTimeDelimiterOption::Space,
            "preserve" => DateTimeDelimiterOption::Preserve,
            _ => DateTimeDelimiterOption::T,
        };

        let toml_version_str: String = get_value(
            &mut config,
            "tomlVersion",
            "v1.0.0".to_string(),
            &mut diagnostics,
        );
        let toml_version = match toml_version_str.as_str() {
            "v1.1.0" | "1.1.0" => TomlVersionOption::V1_1_0,
            _ => TomlVersionOption::V1_0_0,
        };

        let resolved = Configuration {
            line_width: get_value(
                &mut config,
                "lineWidth",
                global_config.line_width.unwrap_or(80) as u8,
                &mut diagnostics,
            ),
            indent_style,
            indent_width: get_value(
                &mut config,
                "indentWidth",
                global_config.indent_width.unwrap_or(2) as u8,
                &mut diagnostics,
            ),
            line_ending,
            string_quote_style,
            date_time_delimiter,
            indent_sub_tables: get_value(&mut config, "indentSubTables", false, &mut diagnostics),
            indent_table_key_value_pairs: get_value(
                &mut config,
                "indentTableKeyValuePairs",
                false,
                &mut diagnostics,
            ),
            key_value_equals_sign_alignment: get_value(
                &mut config,
                "keyValueEqualsSignAlignment",
                false,
                &mut diagnostics,
            ),
            trailing_comment_alignment: get_value(
                &mut config,
                "trailingCommentAlignment",
                false,
                &mut diagnostics,
            ),
            key_value_equals_sign_space_width: get_value(
                &mut config,
                "keyValueEqualsSignSpaceWidth",
                1,
                &mut diagnostics,
            ),
            trailing_comment_space_width: get_value(
                &mut config,
                "trailingCommentSpaceWidth",
                2,
                &mut diagnostics,
            ),
            array_bracket_space_width: get_value(
                &mut config,
                "arrayBracketSpaceWidth",
                0,
                &mut diagnostics,
            ),
            array_comma_space_width: get_value(
                &mut config,
                "arrayCommaSpaceWidth",
                1,
                &mut diagnostics,
            ),
            inline_table_brace_space_width: get_value(
                &mut config,
                "inlineTableBraceSpaceWidth",
                1,
                &mut diagnostics,
            ),
            inline_table_comma_space_width: get_value(
                &mut config,
                "inlineTableCommaSpaceWidth",
                1,
                &mut diagnostics,
            ),
            toml_version,
        };

        diagnostics.extend(get_unknown_property_diagnostics(config));

        PluginResolveConfigurationResult {
            config: resolved,
            diagnostics,
            file_matching: FileMatchingInfo {
                file_extensions: vec!["toml".to_string()],
                file_names: vec![],
            },
        }
    }

    fn check_config_updates(
        &self,
        _message: CheckConfigUpdatesMessage,
    ) -> Result<Vec<ConfigChange>> {
        Ok(vec![])
    }

    fn format(
        &mut self,
        request: SyncFormatRequest<Configuration>,
        _format_with_host: impl FnMut(SyncHostFormatRequest) -> FormatResult,
    ) -> FormatResult {
        let config = request.config;
        let file_text = String::from_utf8_lossy(&request.file_bytes);

        // Format using Tombi
        let result = format_toml(&file_text, config);

        match result {
            Ok(formatted) => {
                if formatted == file_text {
                    Ok(None) // No changes
                } else {
                    Ok(Some(formatted.into_bytes()))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Formatting error: {}", e)),
        }
    }
}

/// Format TOML content using Tombi's formatter.
fn format_toml(source: &str, config: &Configuration) -> Result<String, String> {
    use tombi_formatter::Formatter;
    use tombi_schema_store::SchemaStore;

    // Create schema store (empty for basic formatting)
    let schema_store = SchemaStore::new();
    let format_options = config.to_format_options();
    let toml_version = config.toml_version();

    // Create formatter
    let formatter = Formatter::new(
        toml_version,
        &format_options,
        None, // No source path for dprint
        &schema_store,
    );

    // Tombi's format is async. Handle differently for native vs wasm.
    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async { formatter.format(source).await })
            .map_err(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|d| d.message().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
    }

    #[cfg(target_arch = "wasm32")]
    {
        // For Wasm, we need a sync formatting path or use wasm-bindgen-futures
        // TODO: Implement proper Wasm async handling
        // For now, this is a placeholder that will need Tombi's wasm support
        Err("Wasm async formatting requires additional setup".to_string())
    }
}
