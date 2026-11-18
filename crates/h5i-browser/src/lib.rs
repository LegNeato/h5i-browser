//! h5i browser: a lightweight visual browser engine for coding agents.

pub mod broker;
pub mod budget;
pub mod canvas;
pub mod capture;
#[cfg(feature = "cli")]
pub mod cli;
pub mod cookies;
pub mod cors;
#[cfg(feature = "native")]
pub mod dom_inject;
mod dom_query;
pub mod edits;
pub mod encoding;
pub mod engine;
pub mod extract;
pub mod fonts;
#[cfg(feature = "native")]
pub mod grpc;
pub mod hints;
#[cfg(feature = "identity")]
pub mod identity;
#[cfg(feature = "native")]
pub mod ipc;
pub mod keys;
pub mod markdown;
pub mod multipart;
pub mod net;
pub mod policy;
#[cfg(feature = "native")]
pub mod rawsock;
pub mod read_ir;
pub mod receipt;
pub mod replay;
#[cfg(feature = "javascript")]
pub mod script;
pub mod secrets;
pub mod selector;
pub mod snapshot;
#[cfg(feature = "native")]
pub mod sse;
#[cfg(feature = "cli")]
pub mod stream;
pub mod structured;
pub mod transcript;
pub mod verbs;
#[cfg(feature = "native")]
pub mod ws;
pub mod wsclient;

pub use broker::Broker;
pub use engine::{Page, PageFactory, PageOptions};
#[cfg(feature = "identity")]
pub use identity::Identity;
pub use policy::{Policy, Verdict};
#[cfg(feature = "native")]
pub use receipt::JsonlSink;
pub use receipt::{MemorySink, RequestRecord, Sink};

/// What this engine can and cannot do, answered rather than guessed at.
///
/// h5i reads this instead of inferring capability from a version number: a
/// caller that needs `<video>` should be told so by the engine, and routed to
/// the Chromium path (roadmap-history.md 7.1), not left to discover a blank frame.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    pub engine: String,
    pub version: String,
    /// Whether *this process* will run page script.
    ///
    /// Reported from the running configuration rather than from what the binary
    /// is capable of, because the answer h5i routes on is "will this engine run
    /// the page", not "could it". Script is opt-in (`--script`), and ROADMAP
    /// §12.5 is why: turning it on changes the box's threat model, so it is a
    /// decision someone makes rather than a default they inherit.
    pub javascript: bool,
    pub screenshot: bool,
    pub snapshot: bool,
    /// A live view h5i's existing viewers can attach to, frames driven by
    /// change rather than by a clock.
    pub live_view: bool,
    /// Deliberately absent, and owned by the Chromium path instead.
    pub video: bool,
    /// Timed text off a page's media: `<track>` fetched and parsed, not decoded.
    pub captions: bool,
    pub webgl: bool,
    pub downloads: bool,
    /// Canvas 2D that actually rasterises, and composites into the page.
    pub canvas_2d: bool,
    /// Real WebSocket connections, `ws://` and `wss://`, every frame receipted.
    pub websockets: bool,
    /// Fetches are refused unless a receipts sink is accepting writes.
    pub fail_closed_receipts: bool,
}

impl Capabilities {
    pub fn current() -> Self {
        Self::with_script(false)
    }

    /// What this engine can do with script either on or off.
    pub fn with_script(javascript: bool) -> Self {
        let javascript = javascript && cfg!(feature = "javascript");
        Self {
            engine: "h5i-browser".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            javascript,
            screenshot: true,
            snapshot: true,
            live_view: cfg!(feature = "cli"),
            video: false,
            captions: true,
            webgl: false,
            downloads: false,
            canvas_2d: javascript,
            websockets: javascript && cfg!(feature = "native"),
            fail_closed_receipts: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_do_not_overclaim() {
        let caps = Capabilities::current();
        // Every one of these is a promise h5i routes on. Tier 1 has no script
        // engine linked, so claiming otherwise would send the wrong pages here.
        assert!(!caps.javascript);
        assert!(!caps.video);
        assert!(!caps.webgl);
        assert!(caps.screenshot);
        assert!(caps.fail_closed_receipts);

        // Timed text is not video, and the two are separate claims on purpose:
        // this engine can read what a talk *says* while still not playing it,
        // and a caller routing on `video` must not be told otherwise.
        assert!(caps.captions);
        assert!(!caps.video);

        // Canvas and sockets need a realm to be reachable at all, so with
        // script off they are absent. Promising them here would route a
        // chart-drawing page to an engine that would hand back a blank one.
        assert!(!caps.canvas_2d);
        assert!(!caps.websockets);

        let scripted = Capabilities::with_script(true);
        assert_eq!(scripted.javascript, cfg!(feature = "javascript"));
        assert_eq!(scripted.canvas_2d, cfg!(feature = "javascript"));
        assert_eq!(
            scripted.websockets,
            cfg!(all(feature = "javascript", feature = "native"))
        );
        assert_eq!(scripted.live_view, cfg!(feature = "cli"));
        // And the ones that are absent whatever the configuration stay absent.
        assert!(!scripted.webgl);
        assert!(!scripted.video);
    }
}
