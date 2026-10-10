use crate::{Error, Private};
use cfbk::cost::Amount;
use std::future::Future;
use zeroize::Zeroizing;
/// Trusted HTTP transport request. Never log its URL/body/key.
pub struct HttpRequest {
    /// Adapter-validated HTTPS provider URL.
    pub url: String,
    /// Explicit injected subscription key.
    pub key: Zeroizing<String>,
    /// Private JSON request body.
    pub body: Private,
}
impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HttpRequest([redacted])")
    }
}
/// Trusted transport response; arbitrary member JSON must not implement this port.
pub struct HttpResponse {
    /// Actual HTTP status.
    pub status: u16,
    /// Available response headers as private JSON name/byte-array pairs. Duplicate
    /// names and non-UTF8 bytes are preserved; use an empty list when unavailable.
    pub headers: Private,
    /// Exact bounded response bytes, retained only in RAM.
    pub body: Private,
}
/// HTTPS/authenticity is a trusted transport responsibility. Implementations must
/// not retry, redirect, persist, log private buffers or turn an ambiguous network
/// result into a safe failure. Dummy fixtures are only for tests/simulation.
pub trait Transport: Sync {
    /// Perform exactly one authenticated POST, without hidden retries.
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, Error>> + Send;
}
/// Caller-owned atomic operation journal and shared budget reservation. Refuse
/// any previously dispatched operation (even an exact retry) until the caller
/// reconciles it. Bind the operation and quote atomically and preserve uncertain
/// reservations across crashes. Only opaque references and cost bounds belong
/// here; no raw biometric/provider result is passed to this port.
pub trait OperationStore: Sync {
    /// Reserve operation identity and maximum cost before transport dispatch.
    /// Check the host's fresh clock against this exclusive deadline before success.
    fn reserve(
        &self,
        operation: &str,
        binding: &str,
        maximum: &Amount,
        valid_until: u64,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}
/// Maintained reqwest HTTPS transport. Native only; WASM callers supply their
/// trusted transport explicitly instead of browser-managed opaque redirects.
#[cfg(not(target_arch = "wasm32"))]
pub struct ReqwestTransport(reqwest::Client);
#[cfg(not(target_arch = "wasm32"))]
impl ReqwestTransport {
    /// Build without redirects, implicit proxies, or retries; bounded timeout.
    pub fn new() -> Result<Self, Error> {
        reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map(Self)
            .map_err(|_| Error::Configuration)
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Transport for ReqwestTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, Error> {
        let mut key = reqwest::header::HeaderValue::from_str(&request.key)
            .map_err(|_| Error::Configuration)?;
        key.set_sensitive(true);
        let mut response = self
            .0
            .post(request.url)
            .header("Ocp-Apim-Subscription-Key", key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(request.body.expose().to_vec())
            .send()
            .await
            .map_err(|_| Error::Uncertain)?;
        let status = response.status().as_u16();
        let mut body = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Uncertain)? {
            if chunk.len() > 1_048_576 - body.len() {
                return Err(Error::Uncertain);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            headers: Private::new(
                serde_json::to_vec(
                    &response
                        .headers()
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.as_bytes()))
                        .collect::<Vec<_>>(),
                )
                .expect("HTTP header fields serialize"),
            ),
            body: Private::new(std::mem::take(&mut *body)),
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "tests/transport.rs"]
mod tests;
