//! Library surface of the `charset` VGI worker.
//!
//! The binary (`main.rs`) is the actual worker; this `lib` target exposes the
//! pure detection/transcoding engine so integration tests under `tests/` can
//! exercise it directly, without Arrow or RPC. See [`charset`] for the engine.

pub mod charset;
