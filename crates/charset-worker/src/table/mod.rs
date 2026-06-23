//! Table functions exposed by the charset worker, registered under `charset.main`.

mod supported;

use vgi::Worker;

/// Register every table function on the worker.
pub fn register(worker: &mut Worker) {
    worker.register_table(supported::SupportedEncodings);
}
