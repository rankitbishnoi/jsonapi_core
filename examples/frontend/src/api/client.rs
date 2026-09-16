use gloo_net::http::{Method, RequestBuilder};
use jsonapi_core::Document;
use jsonapi_showcase_resources::ArticleResource;

use super::capture::{MEDIA_TYPE, pretty_body, request_id};
use crate::config::api_base;
use crate::inspector::model::{ExchangeInit, Header};
use crate::inspector::store::InspectorStore;

/// Result of a captured call: the raw response body (already recorded in the
/// inspector) plus the HTTP status, for pages that want to react to it.
#[derive(Clone, Debug)]
pub struct CallResult {
    pub status: u16,
    pub body: String,
}

/// `Copy` client handle held in context. Every call taps the inspector.
#[derive(Clone, Copy)]
pub struct ApiClient {
    store: InspectorStore,
}

impl ApiClient {
    pub fn new(store: InspectorStore) -> Self {
        Self { store }
    }

    fn now_ms() -> f64 {
        // js_sys::Date::now() is always available without additional web-sys features.
        js_sys::Date::now()
    }

    /// Perform a request, record the full exchange, and return status + body.
    pub async fn send(&self, method: &str, path: &str, body: Option<String>) -> CallResult {
        let url = format!("{}{}", api_base(), path);
        // Mirror exactly what the builder sends below: Content-Type is only set
        // when there is a body, so the inspector must not claim otherwise.
        let mut req_headers = vec![Header {
            name: "accept".into(),
            value: MEDIA_TYPE.into(),
        }];
        if body.is_some() {
            req_headers.push(Header {
                name: "content-type".into(),
                value: MEDIA_TYPE.into(),
            });
        }

        let start = Self::now_ms();
        let mut builder = RequestBuilder::new(&url).method(parse_method(method));
        builder = builder.header("Accept", MEDIA_TYPE);

        let sent = if let Some(ref b) = body {
            builder = builder.header("Content-Type", MEDIA_TYPE);
            builder.body(b.clone())
        } else {
            builder.build()
        };

        let (status, resp_headers, resp_body) = match sent {
            Ok(request) => match request.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let headers: Vec<Header> = resp
                        .headers()
                        .entries()
                        .map(|(name, value)| Header { name, value })
                        .collect();
                    let text = resp.text().await.unwrap_or_default();
                    (status, headers, text)
                }
                Err(e) => (
                    0u16,
                    vec![],
                    serde_json::json!({ "network-error": e.to_string() }).to_string(),
                ),
            },
            Err(e) => (
                0u16,
                vec![],
                serde_json::json!({ "build-error": e.to_string() }).to_string(),
            ),
        };
        let duration_ms = (Self::now_ms() - start).max(0.0);

        self.store.record(ExchangeInit {
            method: method.to_string(),
            url,
            req_headers,
            req_body: body.and_then(|b| pretty_body(&b)),
            status,
            request_id: request_id(&resp_headers),
            resp_headers,
            resp_body: pretty_body(&resp_body),
            duration_ms,
        });

        CallResult {
            status,
            body: resp_body,
        }
    }

    /// Convenience GET.
    pub async fn get(&self, path: &str) -> CallResult {
        self.send("GET", path, None).await
    }

    /// Typed helper: dogfood `jsonapi_core` by parsing a list of articles into
    /// `Vec<ArticleResource>`. Returns the parsed titles for a page summary.
    pub async fn list_article_titles(&self, path: &str) -> Result<Vec<String>, String> {
        let res = self.get(path).await;
        let doc: Document<ArticleResource> =
            Document::from_slice(res.body.as_bytes()).map_err(|e| e.to_string())?;
        let items = doc.into_many().map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(|a| a.title).collect())
    }
}

fn parse_method(m: &str) -> Method {
    match m {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        "PUT" => Method::PUT,
        _ => Method::GET,
    }
}
