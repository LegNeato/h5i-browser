//! Channel events shared by native and embedded brokers.

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::Socket;

/// What a socket hands the page.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Open,
    Message(String),
    /// A message carrying an event name, which only server-sent events have.
    ///
    /// A separate variant rather than a marker inside the payload. The first
    /// attempt packed the name into the string as a first line and let the
    /// prelude guess which messages had one, so a plain multi-line
    /// `data: one\ndata: two` was read as an event *named* `one` carrying
    /// `two`, and the page's `onmessage` never fired.
    Named {
        name: String,
        data: String,
    },
    /// The peer or the transport ended it. Carries a reason for the page's
    /// `close` event, and for the console.
    Closed(String),
    /// Something went wrong that the page should see as an `error`.
    Failed(String),
}

/// Which way a frame went, for the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Send,
    Receive,
    /// An event on a server-sent stream.
    ///
    /// Its own variant because the receipt has to say what actually crossed the
    /// wire. An `EventSource` opened as `SSE-OPEN` and then recorded every
    /// event as `WS-RECV`, which describes a WebSocket frame on a connection
    /// that is not one. A log that misdescribes the protocol, in an engine
    /// whose claim is that the log is the truth about the wire.
    StreamReceive,
}

impl Direction {
    /// The method a frame is recorded under.
    ///
    /// Hyphenated so it cannot be mistaken for an HTTP verb by anything reading
    /// the log. A frame is not a request, but "a thing that crossed the wire,
    /// this size, in this direction" is exactly what a `RequestRecord` holds,
    /// and reusing it means every existing reader shows socket traffic without
    /// being taught anything.
    pub fn as_method(self) -> &'static str {
        match self {
            Direction::Send => "WS-SEND",
            Direction::Receive => "WS-RECV",
            Direction::StreamReceive => "SSE-RECV",
        }
    }
}
