//! The tokio runtime shared by the whole process.
//!
//! The server runs on it, and the dispatcher loop and the CLI commands
//! (which are plain synchronous code on their own threads) drive the async HTTP
//! clients through [`block_on`]. A single runtime means a single connection pool:
//! a `reqwest::Client` is tied to the runtime it was created in, so sharing one
//! client across several runtimes would break its idle connections.

use std::sync::OnceLock;
use tokio::runtime::Runtime;

pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("tokio runtime should be created"))
}

/// Runs the given future to completion on the shared runtime.
///
/// Only for synchronous contexts (the CLI and the dispatcher thread);
/// calling this from a runtime thread would panic.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    runtime().block_on(future)
}
