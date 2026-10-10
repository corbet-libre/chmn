use crate::{Error, identifier};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::{fmt, future::Future};
use zeroize::Zeroizing;

/// Stable caller identity key, distinct from rotatable provider API credentials.
pub struct IdentityKey(Zeroizing<[u8; 32]>);
impl IdentityKey {
    /// Import explicit stable key material. Preserve it while comparisons matter.
    pub fn new(key: [u8; 32]) -> Result<Self, Error> {
        if key == [0; 32] {
            return Err(Error::Configuration);
        }
        Ok(Self(Zeroizing::new(key)))
    }
    pub(crate) fn derive(
        &self,
        domain: &str,
        identity: &str,
        scope: &str,
        expires_at: u64,
    ) -> Result<ScopedIdentity, Error> {
        for s in [domain, identity, scope] {
            identifier(s)?;
        }
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(self.0.as_ref())
            .expect("HMAC accepts keys of any length");
        mac.update(b"chmn:identity:v1\0");
        for s in [domain, scope, identity] {
            mac.update(&(s.len() as u64).to_be_bytes());
            mac.update(s.as_bytes());
        }
        Ok(ScopedIdentity {
            bytes: mac.finalize().into_bytes().into(),
            expires_at,
        })
    }
}
/// A comparison value for one authenticated provider identity domain and caller scope.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedIdentity {
    bytes: [u8; 32],
    expires_at: u64,
}
impl ScopedIdentity {
    /// Explicit opaque bytes for caller storage, never a raw biometric provider identifier.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
    /// Atomically reserve up to a caller-selected maximum. Window/release semantics
    /// are encoded by the caller's policy scope, not invented by this crate.
    pub async fn register<S: RegistrationStore>(
        &self,
        store: &S,
        request: &Registration,
        clock: &impl Fn() -> u64,
    ) -> Result<(), Error> {
        let now = clock();
        if now >= self.expires_at {
            return Err(Error::Rejected);
        }
        identifier(&request.policy_scope)?;
        identifier(&request.operation)?;
        if request.maximum == 0 {
            return Err(Error::Configuration);
        }
        store
            .reserve(
                self,
                &request.policy_scope,
                request.maximum,
                &request.operation,
                self.expires_at,
            )
            .await?;
        let completed = clock();
        if completed < now || completed >= self.expires_at {
            return Err(Error::Rejected);
        }
        Ok(())
    }
}
/// Caller-defined registration policy and idempotent operation.
pub struct Registration {
    /// Opaque policy scope including any caller-defined window/generation.
    pub policy_scope: String,
    /// General maximum X, not hard-coded uniqueness.
    pub maximum: u64,
    /// Stable operation ID bound to the actual registration.
    pub operation: String,
}
/// Trusted atomic registration port. Changed maximum/terms for an existing
/// operation must refuse; exact retries are idempotent; competing claims cannot
/// exceed X. No direct database or default volatile implementation is supplied.
pub trait RegistrationStore: Sync {
    /// Check the trusted current time against `valid_until` inside the atomic
    /// commit, including after queue waits; expiry must leave state unchanged.
    /// Atomically reserve without storing raw provider identity data.
    fn reserve(
        &self,
        identity: &ScopedIdentity,
        policy_scope: &str,
        maximum: u64,
        operation: &str,
        valid_until: u64,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}
impl fmt::Debug for IdentityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityKey([redacted])")
    }
}
impl fmt::Debug for ScopedIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScopedIdentity([redacted])")
    }
}
