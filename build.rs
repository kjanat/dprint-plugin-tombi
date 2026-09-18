use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let repository = env::var("CARGO_PKG_REPOSITORY")?;
    let path = repository
        .strip_prefix("https://github.com/")
        .ok_or("package.repository must be a GitHub URL")?;
    let (owner, name) = path.split_once('/').ok_or("missing repository owner")?;
    let short = name
        .strip_prefix("dprint-plugin-")
        .ok_or("repository name must start with dprint-plugin-")?;
    println!("cargo:rustc-env=PLUGIN_PROXY_BASE=https://plugins.dprint.dev/{owner}/{short}");
    println!("cargo:rerun-if-changed=Cargo.toml");
    Ok(())
}
