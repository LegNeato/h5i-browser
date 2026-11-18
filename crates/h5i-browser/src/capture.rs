//! Captured message types and optional native storage.

// The stored shapes are data, and their readers (the websec plugin, recon's
// ledger) have no business linking an engine to name them. They live in
// `h5i-wire`; the writer below stays here, because writing them is the
// engine's job and nobody else's.
pub use h5i_wire::message::{
    Body, Health, MAX_BODY_BYTES, MAX_STORE_BYTES, Skip, StoredRequest, StoredResponse, body_file,
    message_file,
};

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::{Capture, Received, Response};
