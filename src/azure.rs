use crate::{
    Error, HttpRequest, OperationStore, Private, Request, Simulation, Transport, VerifiedMatch,
    identifier,
};
use cfbk::cost::{DispatchPermit, Mode, Price, Quote};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Caller-attested binding of provider face IDs to an authenticated capture.
/// Construct only in the trusted host after its capture/session checks. This is
/// not itself a biometric/liveness proof and cannot be deserialized from a member.
#[derive(Clone, PartialEq, Eq)]
pub struct Capture {
    /// Caller identity domain.
    pub scope: String,
    /// Opaque authenticated caller subject.
    pub subject: String,
    /// Authenticated capture/session reference.
    pub session: String,
    /// Unique operation reference. Retrying a dispatched operation needs host reconciliation.
    pub operation: String,
    /// Face IDs that the trusted capture established for this subject.
    pub faces: BTreeSet<Uuid>,
    /// Exclusive validity supplied by the trusted host.
    pub expires_at: u64,
}
impl Capture {
    pub(crate) fn validate(&self, now: u64) -> Result<(), Error> {
        for s in [&self.scope, &self.subject, &self.session, &self.operation] {
            identifier(s)?;
        }
        if now >= self.expires_at || self.faces.is_empty() || self.faces.len() > 10 {
            return Err(Error::Refused);
        }
        Ok(())
    }
}
impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Capture([redacted])")
    }
}
/// Azure Face v1.2 operations. Enrollment, capture and provider access approval
/// remain caller responsibilities; no liveness is inferred from face matching.
#[derive(Clone, Serialize)]
#[serde(tag = "operation")]
pub enum Operation {
    /// Compare two face IDs. Both must belong to the authenticated capture.
    Compare {
        /// First provider face ID.
        first: Uuid,
        /// Second provider face ID.
        second: Uuid,
    },
    /// Compare a captured face to an enrolled person in one account/group.
    VerifyPerson {
        /// Authenticated captured face.
        face: Uuid,
        /// Provider person group.
        group: String,
        /// Previously enrolled provider person identifier.
        person: Uuid,
    },
    /// Ask the provider for matching enrolled-person candidates.
    Identify {
        /// Authenticated captured faces; one to ten distinct IDs.
        faces: Vec<Uuid>,
        /// Provider person group.
        group: String,
        /// Requested candidate limit, one to one hundred.
        maximum_candidates: u16,
        /// Caller-selected provider threshold, from zero through one.
        threshold: f64,
    },
}
impl Operation {
    pub(crate) fn validate(&self, capture: &Capture) -> Result<(), Error> {
        let faces = match self {
            Self::Compare { first, second } => {
                if first == second {
                    return Err(Error::Configuration);
                }
                vec![*first, *second]
            }
            Self::VerifyPerson {
                face,
                group,
                person,
            } => {
                group_id(group)?;
                if person.is_nil() {
                    return Err(Error::Configuration);
                }
                vec![*face]
            }
            Self::Identify {
                faces,
                group,
                maximum_candidates,
                threshold,
            } => {
                group_id(group)?;
                confidence(*threshold)?;
                if faces.is_empty()
                    || faces.len() > 10
                    || !(1..=100).contains(maximum_candidates)
                    || faces.iter().collect::<BTreeSet<_>>().len() != faces.len()
                {
                    return Err(Error::Configuration);
                }
                faces.clone()
            }
        };
        if faces
            .iter()
            .any(|f| f.is_nil() || !capture.faces.contains(f))
        {
            return Err(Error::Refused);
        }
        Ok(())
    }
    pub(crate) fn wire(&self) -> (&'static str, serde_json::Value) {
        match self {
            Self::Compare { first, second } => (
                "verify",
                serde_json::json!({"faceId1":first,"faceId2":second}),
            ),
            Self::VerifyPerson {
                face,
                group,
                person,
            } => (
                "verify",
                serde_json::json!({"faceId":face,"personId":person,"personGroupId":group}),
            ),
            Self::Identify {
                faces,
                group,
                maximum_candidates,
                threshold,
            } => (
                "identify",
                serde_json::json!({"faceIds":faces,"personGroupId":group,"maxNumOfCandidatesReturned":maximum_candidates,"confidenceThreshold":threshold}),
            ),
        }
    }
}
pub(crate) fn confidence(value: f64) -> Result<(), Error> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Configuration)
    }
}
fn group_id(s: &str) -> Result<(), Error> {
    if s.is_empty()
        || s.len() > 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        Err(Error::Configuration)
    } else {
        Ok(())
    }
}
/// Trusted configured tariff for this exact operation. Must include possible
/// failed-attempt charges and identify account/region/tier in its source.
#[derive(Clone)]
pub struct Tariff {
    /// Exact charge, defensible maximum, or explicitly unknown.
    pub price: Price,
    /// Caller-verified price provenance/revision; never an invented live price.
    pub source: String,
    /// Exclusive validity.
    pub valid_until: u64,
}
/// Immutable inputs for one provider execution.
pub struct Task {
    /// Requested provider operation.
    pub operation: Operation,
    /// Trusted capture and unique operation binding.
    pub capture: Capture,
    /// Explicit configured price bound for this operation.
    pub tariff: Tariff,
}
/// Explicit Azure Face account and endpoint, with no credential discovery.
pub struct AzureFace {
    account: String,
    endpoint: String,
    key: Zeroizing<String>,
    mode: Mode,
}
impl AzureFace {
    /// Configure a public Azure resource endpoint and injected subscription key.
    /// Only HTTPS resource hosts under cognitiveservices.azure.com are accepted.
    /// Simulation does not use this constructor or credentials.
    pub fn new(account: String, endpoint: &str, key: String, mode: Mode) -> Result<Self, Error> {
        identifier(&account)?;
        let url = url::Url::parse(endpoint).map_err(|_| Error::Configuration)?;
        let host = url.host_str().ok_or(Error::Configuration)?;
        let resource = host
            .strip_suffix(".cognitiveservices.azure.com")
            .ok_or(Error::Configuration)?;
        if mode == Mode::Simulation
            || url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || resource.is_empty()
            || !resource
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || key.len() < 16
            || key.len() > 4096
            || !key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            account,
            endpoint: url.to_string().trim_end_matches('/').into(),
            key: Zeroizing::new(key),
            mode,
        })
    }
    /// Inert exact-request cost preview. No quote endpoint is contacted.
    pub fn preview(
        &self,
        operation: &Operation,
        capture: &Capture,
        tariff: &Tariff,
        now: u64,
    ) -> Result<Quote, Error> {
        capture.validate(now)?;
        operation.validate(capture)?;
        identifier(&tariff.source)?;
        if now >= tariff.valid_until {
            return Err(Error::Refused);
        }
        let wire = serde_json::to_vec(operation).map_err(|_| Error::Configuration)?;
        let mut h = Sha256::new();
        h.update(b"chmn:azure-face:v1\0");
        for p in [
            self.account.as_bytes(),
            self.endpoint.as_bytes(),
            capture.scope.as_bytes(),
            capture.subject.as_bytes(),
            capture.session.as_bytes(),
            capture.operation.as_bytes(),
            &wire,
        ] {
            h.update((p.len() as u64).to_be_bytes());
            h.update(p);
        }
        for face in &capture.faces {
            h.update(face.as_bytes());
        }
        h.update(capture.expires_at.to_be_bytes());
        h.update([if self.mode == Mode::Production { 2 } else { 1 }]);
        Ok(Quote {
            price: tariff.price.clone(),
            source: tariff.source.clone(),
            valid_until: tariff.valid_until.min(capture.expires_at),
            binding: hex::encode(h.finalize()),
        })
    }
    /// Execute once after independent quote validation and caller atomic reserve.
    /// A transport error or malformed success is uncertain: retain the full cost
    /// exposure. There are no automatic retries or fallback calls.
    pub async fn execute<T: Transport, S: OperationStore>(
        &self,
        transport: &T,
        store: &S,
        task: Task,
        permit: DispatchPermit<String>,
        now: u64,
    ) -> Result<VerifiedMatch, Error> {
        let Task {
            operation,
            capture,
            tariff,
        } = task;
        let quote = self.preview(&operation, &capture, &tariff, now)?;
        permit
            .validate(&quote.binding, self.mode, now)
            .map_err(|_| Error::Refused)?;
        if permit.candidate() != &self.account || permit.quote() != &quote {
            return Err(Error::Refused);
        }
        store
            .reserve(
                &capture.operation,
                &quote.binding,
                quote.price.maximum().ok_or(Error::Refused)?,
            )
            .await?;
        let (path, body) = operation.wire();
        let response = transport
            .send(HttpRequest {
                url: format!("{}/face/v1.2/{path}", self.endpoint),
                key: self.key.clone(),
                body: Private::new(serde_json::to_vec(&body).map_err(|_| Error::Configuration)?),
            })
            .await?;
        if response.status != 200 {
            return Err(Error::ProviderResponse {
                status: response.status,
                response: response.body,
            });
        }
        VerifiedMatch::parse(
            self.mode,
            &self.account,
            &self.endpoint,
            operation,
            capture,
            response.body,
        )
        .map_err(|_| Error::Uncertain)
    }
}
impl std::fmt::Debug for AzureFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AzureFace([redacted])")
    }
}
/// Deterministic credential-free transformation of an explicitly untrusted
/// fixture. Returns a different type that cannot enter production registration.
pub fn simulate(
    operation: Operation,
    capture: Capture,
    fixture: Vec<u8>,
    request: &Request,
    now: u64,
) -> Result<Simulation, Error> {
    capture.validate(now)?;
    operation.validate(&capture)?;
    let result = VerifiedMatch::parse(
        Mode::Simulation,
        "simulation",
        "local",
        operation,
        capture.clone(),
        Private::new(fixture),
    )?;
    let projection = result.project(request, &capture, now)?;
    Ok(Simulation {
        matches: projection.matches,
        complete: projection.complete,
    })
}
