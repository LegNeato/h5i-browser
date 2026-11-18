//! Broker data and the resource loader shared by native and embedded engines.

use std::sync::Arc;
use std::time::Duration;

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use url::Url;

use crate::receipt::Initiator;

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{LocalBroker, Presented, ProxyResponse};

/// Default user-agent string for native brokers.
pub const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; h5i-browser/",
    env!("CARGO_PKG_VERSION"),
    "; +https://github.com/h5i-dev/h5i)"
);

/// The native broker's default language header.
///
/// With the `identity` feature enabled, the broker derives this header from
/// the identity's locale. Its JavaScript realm reports the same languages.
pub const NATIVE_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";

/// What a fetch produced. A denied or failed fetch is still an outcome, with
/// an empty body and a reason. Never an absence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchOutcome {
    /// The receipt sequence number this request was recorded under.
    ///
    /// Carried out of the broker so a caller can say *which* receipt a page's
    /// action produced. `None` when the request never reached the point of
    /// being recorded, which is the same thing as there being no receipt to
    /// name.
    pub seq: Option<u64>,
    /// Response headers, in arrival order. Carried because `Response.headers`
    /// is how a page learns content type, pagination links and rate limits, and
    /// returning `null` from `headers.get` made every one of those look absent
    /// rather than unsupported.
    pub headers: Vec<(String, String)>,
    pub final_url: Url,
    /// Carried beside the message rather than inside it when this crosses a
    /// process boundary: a page's images are megabytes, and base64 in JSON
    /// would pay a third again for the privilege of being unreadable.
    #[serde(skip)]
    pub body: Vec<u8>,
    pub status: Option<u16>,
    pub error: Option<String>,
    /// Set when this was a cross-origin `no-cors` request.
    ///
    /// The body and headers are already empty and the status is already zero,
    /// which is what an opaque response *is*. The flag exists so the caller can
    /// say so rather than presenting a page with an empty 0 that looks like a
    /// failure. A page checking `response.type === "opaque"` is doing the
    /// right thing and should get the right answer.
    pub opaque: bool,
}

impl FetchOutcome {
    /// An outcome that never reached the wire. Public because the script realm
    /// needs to answer a request it could not even start, rather than leaving
    /// the page's promise pending forever.
    pub fn refused(url: Url, error: String) -> Self {
        Self::failed(url, error)
    }

    pub(crate) fn failed(url: Url, error: String) -> Self {
        Self::failed_at(url, error, None)
    }

    pub(crate) fn failed_at(url: Url, error: String, seq: Option<u64>) -> Self {
        Self {
            seq,
            headers: Vec::new(),
            final_url: url,
            body: Vec::new(),
            status: None,
            error: Some(error),
            opaque: false,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// The three headers a client must compute for itself.
///
/// They frame the message: a `Content-Length` that disagrees with the body, or a
/// `Transfer-Encoding` this client is not using, describes a request other than
/// the one that goes out. A caller setting one is refused and told, which is a
/// different answer from being quietly obeyed by a client that then computes the
/// real value anyway. Request smuggling is a real thing to test for, and an
/// engine that pretended to support it here would be lying about what it sent.
pub fn header_is_the_clients(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "content-length" | "transfer-encoding" | "connection"
    )
}

/// One gRPC call for [`crate::broker::Broker::grpc_call`]. The body arrives framed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GrpcAsk {
    pub url: String,
    pub path: String,
    pub authority: Option<String>,
    pub body: Vec<u8>,
    pub metadata: Vec<(String, String)>,
    pub insecure: bool,
    pub timeout: Option<Duration>,
}

/// What a gRPC call yielded, whether it was refused, failed, or answered.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GrpcCallOutcome {
    /// The receipt id it was recorded under, when it reached the recording gate.
    pub seq: Option<u64>,
    /// The policy's reason, when the call was refused before it went out.
    pub denied: Option<String>,
    pub http_status: Option<u16>,
    pub grpc_status: Option<i32>,
    pub grpc_message: Option<String>,
    /// The framed response body.
    pub body: Vec<u8>,
    /// A transport error, when the call went out but did not complete.
    pub error: Option<String>,
}

#[cfg(feature = "native")]
impl GrpcCallOutcome {
    /// A failure before a receipt was minted (a bad URL, an unwritable log).
    fn error(message: String) -> Self {
        GrpcCallOutcome {
            error: Some(message),
            ..GrpcCallOutcome::default()
        }
    }
}

/// How every subresource turned out, for the page's `load` and `error` events.
///
/// Blitz decides each outcome and throws it away, so nothing in the page could
/// see the 404 sitting in the request log. Keyed by URL because that is all the
/// two sides share: Blitz's `Request` does not say which element asked.
pub type ResourceLog = Arc<std::sync::Mutex<ResourceOutcomes>>;

/// The table behind a [`ResourceLog`].
#[derive(Debug, Default)]
pub struct ResourceOutcomes {
    /// `0` for a request that got no answer: refused, or a failed connection.
    /// Both are `error` to a page; which it was is in the receipt.
    by_url: std::collections::HashMap<String, u16>,
}

impl ResourceOutcomes {
    /// Under both URLs: a redirected image is `src` in the markup and the final
    /// URL in the outcome, and the page asks about the first.
    pub fn record(&mut self, asked: &Url, outcome: &FetchOutcome) {
        let status = match (outcome.error.as_ref(), outcome.status) {
            (Some(_), _) | (None, None) => 0,
            (None, Some(status)) => status,
        };
        self.by_url.insert(asked.to_string(), status);
        self.by_url.insert(outcome.final_url.to_string(), status);
    }

    /// The status this URL came back with, or `None` if it was never asked for.
    pub fn status(&self, url: &str) -> Option<u16> {
        self.by_url.get(url).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.by_url.is_empty()
    }
}

/// Adapts the broker to Blitz's [`NetProvider`].
pub struct BrokerNet {
    broker: Arc<dyn crate::broker::Broker>,
    /// The document whose subresources these are.
    ///
    /// Load-bearing, not bookkeeping. Every image, stylesheet and font on the
    /// page arrives through here, and without an origin to attribute them to
    /// the policy read each one as the agent naming a URL, so
    /// `<img src="http://127.0.0.1:3000/…">` on a page from the open web reached
    /// the box's dev server, which is precisely what [`crate::policy::Policy::check_from`]
    /// exists to refuse. `None` only for a document with no origin of its own.
    document: Option<Url>,
    /// Where each subresource's fate is written down for the page to hear.
    resources: ResourceLog,
}

impl BrokerNet {
    pub fn new(broker: Arc<dyn crate::broker::Broker>, document: Option<Url>) -> Self {
        Self::with_log(broker, document, ResourceLog::default())
    }

    pub fn with_log(
        broker: Arc<dyn crate::broker::Broker>,
        document: Option<Url>,
        resources: ResourceLog,
    ) -> Self {
        Self {
            broker,
            document,
            resources,
        }
    }
}

impl NetProvider for BrokerNet {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let outcome =
            self.broker
                .fetch_from(&request.url, Initiator::Subresource, self.document.as_ref());

        if let Ok(mut log) = self.resources.lock() {
            log.record(&request.url, &outcome);
        }

        // The single exit. A denied or failed request completes with an empty
        // body: Blitz counts the resource as resolved and paints, having
        // loaded nothing. Returning early here, however tempting for a
        // request we refused, leaves the document pending forever and blank.
        handler.bytes(outcome.final_url.to_string(), Bytes::from(outcome.body));
    }
}

#[cfg(test)]
mod tests;
