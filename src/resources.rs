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
