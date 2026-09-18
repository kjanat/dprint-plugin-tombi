use serde::Serialize;
use tombi_config::{
    ArrayBracketSpaceWidth, ArrayCommaSpaceWidth, DateTimeDelimiter, IndentStyle, IndentWidth,
    InlineTableBraceSpaceWidth, InlineTableCommaSpaceWidth, KeyValueEqualsSignSpaceWidth,
    LineEnding, LineWidth, StringQuoteStyle, TrailingCommentSpaceWidth,
};

/// Configuration for the Tombi TOML formatter plugin.
///
/// Maps dprint configuration keys to Tombi's `FormatOptions`.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schema",
    schemars(
        title = "dprint-plugin-tombi Configuration",
        description = "TOML formatter plugin using Tombi"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    /// The maximum line width the formatter will try to stay under.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 1, max = 255), default = "default_line_width")
    )]
    pub line_width: u8,

    /// Whether to use tabs or spaces for indentation.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub indent_style: IndentStyleOption,

    /// The number of spaces per indentation level (when using spaces).
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 1, max = 8), default = "default_indent_width")
    )]
    pub indent_width: u8,

    /// The type of line ending to use.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub line_ending: LineEndingOption,

    /// The preferred quote style for strings.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub string_quote_style: StringQuoteStyleOption,

    /// The delimiter between date and time in datetime values.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub date_time_delimiter: DateTimeDelimiterOption,

    /// Whether to indent sub-tables relative to their parent table.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub indent_sub_tables: bool,

    /// Whether to indent key-value pairs within tables.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub indent_table_key_value_pairs: bool,

    /// Whether to align the equals signs in consecutive key-value pairs.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub key_value_equals_sign_alignment: bool,

    /// Whether to align trailing comments in consecutive lines.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub trailing_comment_alignment: bool,

    /// The number of spaces around the equals sign in key-value pairs.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 0, max = 8), default = "default_key_value_space")
    )]
    pub key_value_equals_sign_space_width: u8,

    /// The number of spaces before a trailing comment.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 1, max = 8), default = "default_trailing_comment_space")
    )]
    pub trailing_comment_space_width: u8,

    /// The number of spaces inside array brackets on a single line.
    #[cfg_attr(feature = "schema", schemars(range(min = 0, max = 8), default))]
    pub array_bracket_space_width: u8,

    /// The number of spaces after commas in single-line arrays.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 0, max = 8), default = "default_one")
    )]
    pub array_comma_space_width: u8,

    /// The number of spaces inside inline table braces.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 0, max = 8), default = "default_one")
    )]
    pub inline_table_brace_space_width: u8,

    /// The number of spaces after commas in inline tables.
    #[cfg_attr(
        feature = "schema",
        schemars(range(min = 0, max = 8), default = "default_one")
    )]
    pub inline_table_comma_space_width: u8,

    /// The TOML version to use for parsing and validation.
    #[cfg_attr(feature = "schema", schemars(default))]
    pub toml_version: TomlVersionOption,
}

#[cfg(feature = "schema")]
fn default_line_width() -> u8 {
    80
}

#[cfg(feature = "schema")]
fn default_indent_width() -> u8 {
    2
}

#[cfg(feature = "schema")]
fn default_key_value_space() -> u8 {
    1
}

#[cfg(feature = "schema")]
fn default_trailing_comment_space() -> u8 {
    2
}

#[cfg(feature = "schema")]
fn default_one() -> u8 {
    1
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            line_width: 80,
            indent_style: IndentStyleOption::Space,
            indent_width: 2,
            line_ending: LineEndingOption::Lf,
            string_quote_style: StringQuoteStyleOption::Double,
            date_time_delimiter: DateTimeDelimiterOption::T,
            indent_sub_tables: false,
            indent_table_key_value_pairs: false,
            key_value_equals_sign_alignment: false,
            trailing_comment_alignment: false,
            key_value_equals_sign_space_width: 1,
            trailing_comment_space_width: 2,
            array_bracket_space_width: 0,
            array_comma_space_width: 1,
            inline_table_brace_space_width: 1,
            inline_table_comma_space_width: 1,
            toml_version: TomlVersionOption::V1_0_0,
        }
    }
}

/// Indent style option for dprint configuration.
#[derive(Clone, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum IndentStyleOption {
    /// Use spaces for indentation.
    #[default]
    Space,
    /// Use tabs for indentation.
    Tab,
}

/// Line ending option for dprint configuration.
#[derive(Clone, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum LineEndingOption {
    /// Line Feed only (`\n`), common on Linux and macOS.
    #[default]
    Lf,
    /// Carriage Return Line Feed (`\r\n`), common on Windows.
    Crlf,
}

/// String quote style option for dprint configuration.
#[derive(Clone, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum StringQuoteStyleOption {
    /// Always use double quotes.
    #[default]
    Double,
    /// Always use single quotes.
    Single,
    /// Preserve the existing quote style.
    Preserve,
}

/// Date-time delimiter option for dprint configuration.
#[derive(Clone, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum DateTimeDelimiterOption {
    /// Use `T` as delimiter (e.g., `2021-01-01T00:00:00`).
    #[default]
    T,
    /// Use space as delimiter (e.g., `2021-01-01 00:00:00`).
    #[serde(rename = "space")]
    Space,
    /// Preserve the existing delimiter.
    #[serde(rename = "preserve")]
    Preserve,
}

/// TOML version option for dprint configuration.
#[derive(Clone, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum TomlVersionOption {
    /// TOML v1.0.0 (stable).
    #[default]
    #[serde(rename = "v1.0.0")]
    V1_0_0,
    /// TOML v1.1.0 (preview).
    #[serde(rename = "v1.1.0")]
    V1_1_0,
}

impl Configuration {
    /// Convert to Tombi's `FormatOptions`
    #[must_use]
    pub fn to_format_options(&self) -> tombi_config::FormatOptions {
        use tombi_config::format::FormatRules;

        tombi_config::FormatOptions {
            rules: Some(FormatRules {
                line_width: LineWidth::try_from(self.line_width).ok(),
                indent_style: Some(match self.indent_style {
                    IndentStyleOption::Tab => IndentStyle::Tab,
                    IndentStyleOption::Space => IndentStyle::Space,
                }),
                indent_width: Some(IndentWidth::from(self.indent_width)),
                line_ending: Some(match self.line_ending {
                    LineEndingOption::Crlf => LineEnding::Crlf,
                    LineEndingOption::Lf => LineEnding::Lf,
                }),
                string_quote_style: Some(match self.string_quote_style {
                    StringQuoteStyleOption::Single => StringQuoteStyle::Single,
                    StringQuoteStyleOption::Preserve => StringQuoteStyle::Preserve,
                    StringQuoteStyleOption::Double => StringQuoteStyle::Double,
                }),
                date_time_delimiter: Some(match self.date_time_delimiter {
                    DateTimeDelimiterOption::Space => DateTimeDelimiter::Space,
                    DateTimeDelimiterOption::Preserve => DateTimeDelimiter::Preserve,
                    DateTimeDelimiterOption::T => DateTimeDelimiter::T,
                }),
                indent_sub_tables: Some(self.indent_sub_tables),
                indent_table_key_value_pairs: Some(self.indent_table_key_value_pairs),
                key_value_equals_sign_alignment: Some(self.key_value_equals_sign_alignment),
                trailing_comment_alignment: Some(self.trailing_comment_alignment),
                key_value_equals_sign_space_width: Some(KeyValueEqualsSignSpaceWidth::from(
                    self.key_value_equals_sign_space_width,
                )),
                trailing_comment_space_width: Some(TrailingCommentSpaceWidth::from(
                    self.trailing_comment_space_width,
                )),
                array_bracket_space_width: Some(ArrayBracketSpaceWidth::from(
                    self.array_bracket_space_width,
                )),
                array_comma_space_width: Some(ArrayCommaSpaceWidth::from(
                    self.array_comma_space_width,
                )),
                inline_table_brace_space_width: Some(InlineTableBraceSpaceWidth::from(
                    self.inline_table_brace_space_width,
                )),
                inline_table_comma_space_width: Some(InlineTableCommaSpaceWidth::from(
                    self.inline_table_comma_space_width,
                )),
            }),
        }
    }

    /// Get the TOML version
    #[must_use]
    pub const fn toml_version(&self) -> tombi_toml_version::TomlVersion {
        match self.toml_version {
            TomlVersionOption::V1_1_0 => tombi_toml_version::TomlVersion::V1_1_0,
            TomlVersionOption::V1_0_0 => tombi_toml_version::TomlVersion::V1_0_0,
        }
    }
}
