use std::{collections::BTreeMap, sync::Arc};

use bytes::Bytes;
use tombi_schema_store::{FetchError, HttpClient, HttpFuture};

/// Serves the files kprint already downloaded. Missing files are errors.
#[derive(Debug)]
pub struct Resources {
    files: BTreeMap<String, String>,
    schema_sources: BTreeMap<String, String>,
}

impl HttpClient for Resources {
    fn get_bytes<'a>(&'a self, url: &'a str) -> HttpFuture<'a, Result<Bytes, FetchError>> {
        Box::pin(async move {
            let text = self.files.get(url).ok_or_else(|| FetchError::FetchFailed {
                reason: format!("kprint did not supply {url}"),
            })?;
            match self
                .schema_sources
                .get(url)
                .filter(|destination| *destination != url)
            {
                Some(destination) => {
                    redirect_schema(text, destination)
                        .map(Bytes::from)
                        .map_err(|error| FetchError::FetchFailed {
                            reason: format!("Loading redirected schema {url}: {error}"),
                        })
                }
                None => Ok(Bytes::copy_from_slice(text.as_bytes())),
            }
        })
    }
}

pub fn client(files: BTreeMap<String, String>) -> Arc<dyn HttpClient> {
    Arc::new(Resources {
        files,
        schema_sources: BTreeMap::new(),
    })
}

pub fn schema_client(
    files: BTreeMap<String, String>,
    sources: &BTreeMap<String, String>,
) -> Arc<dyn HttpClient> {
    Arc::new(Resources {
        files,
        schema_sources: sources.clone(),
    })
}

fn redirect_schema(text: &str, destination: &str) -> anyhow::Result<String> {
    let mut schema: serde_json::Value = serde_json::from_str(text)?;
    if let Some(object) = schema.as_object_mut() {
        let base = destination.parse::<tombi_uri::Uri>()?;
        let id = match object.get("$id").and_then(serde_json::Value::as_str) {
            Some(id) => base.join(id)?.to_string(),
            None => destination.to_owned(),
        };
        object.insert("$id".into(), serde_json::Value::String(id));
    }
    Ok(serde_json::to_string(&schema)?)
}
