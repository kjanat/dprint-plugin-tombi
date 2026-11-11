//! A dprint Wasm adapter for the Rust engine underlying `tombi-wasm-lib`.
//! Kprint supplies configuration files and schemas before the plugin formats TOML.

mod configuration;
mod handler;
mod resources;

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
    use dprint_core::generate_plugin_code;

    generate_plugin_code!(TombiPluginHandler, TombiPluginHandler::new());

    unsafe extern "Rust" {
        fn clear_shared_bytes(size: usize) -> *const u8;
    }

    fn set_shared_bytes_str(text: &str) -> usize {
        // Use the exported buffer API; dprint-core keeps its state private.
        // SAFETY: clear_shared_bytes allocates text.len() writable bytes. The
        // source is a separate allocation, and no other buffer call intervenes.
        unsafe {
            let buffer = clear_shared_bytes(text.len()).cast_mut();
            std::ptr::copy_nonoverlapping(text.as_ptr(), buffer, text.len());
        }
        text.len()
    }

    /// Tombi configuration filenames for kprint's automatic discovery.
    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_files() -> usize {
        set_shared_bytes_str(
            &serde_json::to_string(&crate::CONFIG_FILES).expect("a list of strings serializes"),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_reference_paths() -> usize {
        set_shared_bytes_str(r#"[["extends","*"]]"#)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_relative_path_bases() -> usize {
        set_shared_bytes_str(r#"{".config/tombi.toml":".."}"#)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_schema_paths() -> usize {
        set_shared_bytes_str(r#"[["schemas","*","path"]]"#)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn get_config_local_schemas_use_root() -> usize {
        set_shared_bytes_str("true")
    }
}
