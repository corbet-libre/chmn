//! RAM-only biometric provider integration with explicit costs and caller policy.
#![forbid(unsafe_code)]
mod azure;
mod identity;
mod result;
mod transport;
pub use azure::{AzureFace, Capture, Operation, Tariff, Task, simulate};
pub use identity::{IdentityKey, Registration, RegistrationStore, ScopedIdentity};
pub use result::{IdentityRequest, MatchFact, Projection, Request, Simulation, VerifiedMatch};
#[cfg(not(target_arch = "wasm32"))]
pub use transport::ReqwestTransport;
pub use transport::{HttpRequest, HttpResponse, OperationStore, Transport};
use zeroize::Zeroizing;

/// Private RAM buffer. Access is explicit and diagnostics are redacted.
pub struct Private(Zeroizing<Vec<u8>>);
impl Private {
    /// Import a buffer. This does not authenticate its contents.
    pub fn new(value: Vec<u8>) -> Self {
        Self(Zeroizing::new(value))
    }
    /// Explicit private access; caller owns any copies it creates.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl std::fmt::Debug for Private {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Private([redacted])")
    }
}
/// Payload-free failures. A dispatched error does not establish zero cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid configuration or caller request.
    #[error("invalid biometric configuration")]
    Configuration,
    /// Cost, expiry, mode or request binding does not authorize dispatch.
    #[error("biometric dispatch refused")]
    Refused,
    /// The operation may have reached the provider. Keep its cost reservation.
    #[error("biometric outcome uncertain")]
    Uncertain,
    /// Provider returned a definitive non-success HTTP response; it may be billed.
    #[error("biometric provider rejected operation")]
    Provider,
    /// Authenticated provider representation was invalid or not request-bound.
    #[error("biometric response rejected")]
    Rejected,
    /// The requested identity or fact is not established by this result.
    #[error("biometric fact unsupported")]
    Unsupported,
    /// Caller atomic state refused the reservation.
    #[error("biometric state reservation refused")]
    Reservation,
}
pub(crate) fn identifier(s: &str) -> Result<(), Error> {
    if s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control) {
        Err(Error::Configuration)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests;
