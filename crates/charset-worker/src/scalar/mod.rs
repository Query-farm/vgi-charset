//! Scalar functions exposed by the charset worker, registered under `charset.main`.

mod decode;
mod detect;
mod encode;

use vgi::Worker;

/// Register every scalar function on the worker.
pub fn register(worker: &mut Worker) {
    worker.register_scalar(detect::DetectEncoding);
    worker.register_scalar(detect::DetectConfidence);
    worker.register_scalar(detect::IsValidUtf8);
    worker.register_scalar(decode::ToUtf8);
    worker.register_scalar(decode::ToUtf8From);
    worker.register_scalar(encode::Transcode);
    worker.register_scalar(encode::FixMojibake);
}
