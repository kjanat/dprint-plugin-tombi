mod configuration;
mod handler;

pub use configuration::Configuration;
pub use handler::TombiPluginHandler;

// The generate_plugin_code! macro is only available when compiling for wasm32
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod wasm_plugin {
    use crate::Configuration;
    use crate::TombiPluginHandler;
    use dprint_core::generate_plugin_code;
    use dprint_core::plugins::SyncPluginHandler;

    generate_plugin_code!(TombiPluginHandler, TombiPluginHandler);
}
