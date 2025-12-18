//! Generate JSON Schema for dprint-plugin-tombi configuration.
//!
//! Run with: `cargo run --bin generate-schema --features schema`

use dprint_plugin_tombi::Configuration;
use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;

/// Extract version from Cargo.toml at compile time
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = SchemaSettings::draft07();
    let generator = SchemaGenerator::new(settings);

    let schema = generator.into_root_schema_for::<Configuration>();

    // Add $id to the schema with version from Cargo.toml
    let schema_id = format!(
        "https://plugins.dprint.dev/kjanat/dprint-plugin-tombi/{}/schema.json",
        VERSION
    );

    let mut schema_value = serde_json::to_value(&schema)?;
    if let serde_json::Value::Object(ref mut map) = schema_value {
        map.insert("$id".to_string(), serde_json::Value::String(schema_id));
    }

    let schema_json = serde_json::to_string_pretty(&schema_value)? + "\n";

    std::fs::write("schema.json", &schema_json)?;
    println!("Generated schema.json (version {})", VERSION);

    Ok(())
}
