/// POST /v1/passport/authorize — Passport-scoped authorization endpoint.
///
/// Delegates to the core authorization pipeline. The SDK `PassportClient` sends
/// delegation chains here; the handler validates chain, narrowing, nonce, and
/// capability bound, then returns a `ProvableReceipt`.

pub use crate::routes::authorize::handler;
