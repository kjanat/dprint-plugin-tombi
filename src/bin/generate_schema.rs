use std::{env, fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let path = env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("schema.json"), PathBuf::from);
    let mut schema = dprint_plugin_tombi::configuration_schema()?;
    // Upstream enables serde_json/preserve_order. Sort explicitly, including
    // nested definitions, so dependency feature unification cannot cause drift.
    schema.sort_all_objects();
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(&schema)?))?;
    Ok(())
}
