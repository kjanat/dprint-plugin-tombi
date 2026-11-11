use std::{collections::BTreeMap, sync::Arc};

use bytes::Bytes;
use tombi_schema_store::{FetchError, HttpClient, HttpFuture};

/// Serves the files kprint already downloaded. Missing files are errors.
#[derive(Debug)]
pub struct Resources(pub BTreeMap<String, String>);

impl HttpClient for Resources {
    fn get_bytes<'a>(&'a self, url: &'a str) -> HttpFuture<'a, Result<Bytes, FetchError>> {
        Box::pin(async move {
            self.0
                .get(url)
                .map(|text| Bytes::copy_from_slice(text.as_bytes()))
                .ok_or_else(|| FetchError::FetchFailed {
                    reason: format!("kprint did not supply {url}"),
                })
        })
    }
}

pub fn client(files: BTreeMap<String, String>) -> Arc<dyn HttpClient> {
    Arc::new(Resources(files))
}

pub fn schema_client(
    mut files: BTreeMap<String, String>,
    sources: &BTreeMap<String, String>,
) -> anyhow::Result<Arc<dyn HttpClient>> {
    for (requested, destination) in sources {
        if requested == destination {
            continue;
        }
        if let Some(text) = files.get_mut(requested) {
            let mut schema: serde_json::Value = serde_json::from_str(text)?;
            if let Some(object) = schema.as_object_mut() {
                let base = destination.parse::<tombi_uri::Uri>()?;
                let id = match object.get("$id").and_then(serde_json::Value::as_str) {
                    Some(id) => base.join(id)?.to_string(),
                    None => destination.clone(),
                };
                object.insert("$id".into(), serde_json::Value::String(id));
                *text = serde_json::to_string(&schema)?;
            }
        }
    }
    Ok(client(files))
}
