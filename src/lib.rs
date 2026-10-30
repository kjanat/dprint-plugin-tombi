//! A dprint Wasm adapter for the Rust engine underlying `tombi-wasm-lib`.
//! The production plugin is offline and does not use JavaScript bindings.

mod configuration;
mod handler;

#[cfg(feature = "schema")]
pub use configuration::configuration_schema;
pub use configuration::{CONFIG_FILES, Configuration};
pub use handler::TombiPluginHandler;

/// Versioned configuration schema for this exact build.
pub const SCHEMA_URL: &str = concat!(
    env!("PLUGIN_PROXY_BASE"),
    "/",
    env!("CARGO_PKG_VERSION"),
    "/schema.json"
);
/// Update metadata for the plugin's GitHub repository.
pub const UPDATE_URL: &str = concat!(env!("PLUGIN_PROXY_BASE"), "/latest.json");

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[allow(unsafe_code, reason = "dprint owns the generated ABI boundary")]
mod wasm_plugin {
    use crate::{Configuration, TombiPluginHandler};
    use dprint_core::{generate_plugin_code, plugins::SyncPluginHandler};

    generate_plugin_code!(TombiPluginHandler, TombiPluginHandler::new());

    /// The configuration files of Tombi's own for a dprint that passes
    /// plugins one (its `PluginInfo::config_files`, which dprint's
    /// `PluginInfo` here can't have).
    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_files() -> usize {
        set_shared_bytes_str(
            serde_json::to_string(&crate::CONFIG_FILES).expect("a list of strings serializes"),
        )
    }
}
