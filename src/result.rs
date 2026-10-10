use crate::azure::confidence;
use crate::{Capture, Error, IdentityKey, Operation, Private, ScopedIdentity};
use serde::Deserialize;
use uuid::Uuid;

/// Requested provider facts. No name, birth date or liveness is invented.
#[derive(Default)]
pub struct Request {
    /// Explicitly return complete upstream JSON, including extension fields.
    pub complete: bool,
    /// Explicitly return provider confidence scores. Not normalized trust scores.
    pub confidence: bool,
    /// Return provider face references only when explicitly requested.
    pub references: bool,
    /// Caller threshold for a positive projected match.
    pub minimum_confidence: f64,
}
struct Match {
    face: Uuid,
    person: Option<Uuid>,
    matched: bool,
    confidence: f64,
}
/// Authenticated result from the trusted transport for an exact operation.
/// No public constructor accepts arbitrary JSON as a production result.
pub struct VerifiedMatch {
    mode: cfbk::cost::Mode,
    account: String,
    endpoint: String,
    operation: Operation,
    capture: Capture,
    matches: Vec<Match>,
    original: Private,
}
impl VerifiedMatch {
    /// Explicit access to the complete private upstream representation, including
    /// after a requested projection refuses. These bytes do not grant fresh proof.
    pub fn upstream(&self) -> &Private {
        &self.original
    }
    pub(crate) fn parse(
        mode: cfbk::cost::Mode,
        account: &str,
        endpoint: &str,
        operation: Operation,
        capture: Capture,
        original: Private,
    ) -> Result<Self, Error> {
        if original.expose().len() > 1_048_576 {
            return Err(Error::Rejected);
        }
        let matches = match &operation {
            Operation::Compare { first, .. } | Operation::VerifyPerson { face: first, .. } => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Response {
                    is_identical: bool,
                    confidence: f64,
                }
                let response: Response =
                    serde_json::from_slice(original.expose()).map_err(|_| Error::Rejected)?;
                confidence(response.confidence).map_err(|_| Error::Rejected)?;
                let person = match &operation {
                    Operation::VerifyPerson { person, .. } => Some(*person),
                    _ => None,
                };
                vec![Match {
                    face: *first,
                    person,
                    matched: response.is_identical,
                    confidence: response.confidence,
                }]
            }
            Operation::Identify {
                faces,
                maximum_candidates,
                threshold,
                ..
            } => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Candidate {
                    person_id: Uuid,
                    confidence: f64,
                }
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Response {
                    face_id: Uuid,
                    candidates: Vec<Candidate>,
                }
                let response: Vec<Response> =
                    serde_json::from_slice(original.expose()).map_err(|_| Error::Rejected)?;
                if response.len() != faces.len() {
                    return Err(Error::Rejected);
                }
                let mut seen = std::collections::BTreeSet::new();
                let mut matches = Vec::new();
                for entry in response {
                    if !faces.contains(&entry.face_id)
                        || !seen.insert(entry.face_id)
                        || entry.candidates.len() > usize::from(*maximum_candidates)
                    {
                        return Err(Error::Rejected);
                    }
                    if entry.candidates.is_empty() {
                        matches.push(Match {
                            face: entry.face_id,
                            person: None,
                            matched: false,
                            confidence: 0.0,
                        });
                    }
                    let mut persons = std::collections::BTreeSet::new();
                    for candidate in entry.candidates {
                        confidence(candidate.confidence).map_err(|_| Error::Rejected)?;
                        if candidate.person_id.is_nil()
                            || !persons.insert(candidate.person_id)
                            || candidate.confidence < *threshold
                        {
                            return Err(Error::Rejected);
                        }
                        matches.push(Match {
                            face: entry.face_id,
                            person: Some(candidate.person_id),
                            matched: true,
                            confidence: candidate.confidence,
                        });
                    }
                }
                matches
            }
        };
        Ok(Self {
            mode,
            account: account.into(),
            endpoint: endpoint.into(),
            operation,
            capture,
            matches,
            original,
        })
    }
    /// Return only requested facts; freshness and trusted capture must still match.
    pub fn project(
        &self,
        request: &Request,
        capture: &Capture,
        now: u64,
    ) -> Result<Projection, Error> {
        self.check_context(capture, now)?;
        confidence(request.minimum_confidence)?;
        Ok(Projection {
            mode: self.mode,
            matches: self
                .matches
                .iter()
                .map(|m| MatchFact {
                    face: request.references.then_some(m.face),
                    matched: m.matched && m.confidence >= request.minimum_confidence,
                    confidence: request.confidence.then_some(m.confidence),
                })
                .collect(),
            complete: request
                .complete
                .then(|| Private::new(self.original.expose().to_vec())),
        })
    }
    /// Derive a comparison for exactly one qualifying enrolled-person candidate.
    /// This preserves provider account/group semantics, not universal personhood.
    /// Multiple qualifying candidates refuse instead of silently picking a person.
    pub fn comparison(
        &self,
        key: &IdentityKey,
        request: &IdentityRequest,
        capture: &Capture,
        now: u64,
    ) -> Result<ScopedIdentity, Error> {
        self.check_context(capture, now)?;
        if self.mode != cfbk::cost::Mode::Production {
            return Err(Error::Refused);
        }
        confidence(request.minimum_confidence)?;
        let group = match &self.operation {
            Operation::VerifyPerson { group, .. } | Operation::Identify { group, .. } => group,
            _ => return Err(Error::Unsupported),
        };
        let mut found = self.matches.iter().filter(|m| {
            m.face == request.face && m.matched && m.confidence >= request.minimum_confidence
        });
        let person = found
            .next()
            .and_then(|m| m.person)
            .ok_or(Error::Unsupported)?;
        if found.next().is_some() {
            return Err(Error::Unsupported);
        }
        // Structural encoding avoids account/group delimiter collisions.
        let domain = serde_json::json!([
            "azure-face-v1.2",
            &self.account,
            &self.endpoint,
            format!("{:?}", self.mode),
            group,
        ])
        .to_string();
        key.derive(
            &domain,
            &person.to_string(),
            &request.scope,
            self.capture.expires_at,
        )
    }
    fn check_context(&self, capture: &Capture, now: u64) -> Result<(), Error> {
        capture.validate(now)?;
        if capture != &self.capture {
            return Err(Error::Refused);
        }
        Ok(())
    }
}
/// One requested provider fact, without enrolled-person identifiers.
pub struct MatchFact {
    /// Provider face reference belonging to the trusted capture.
    pub face: Option<Uuid>,
    /// Provider comparison/candidate outcome, not a liveness/personhood claim.
    pub matched: bool,
    /// Raw provider confidence only when requested.
    pub confidence: Option<f64>,
}
/// Private domain projection. Complete result access is explicitly requested.
pub struct Projection {
    /// Authenticated execution mode; sandbox results are not production results.
    pub mode: cfbk::cost::Mode,
    /// Provider matching facts; no enrolled-person IDs by default.
    pub matches: Vec<MatchFact>,
    /// Exact provider bytes including extensions, if requested.
    pub complete: Option<Private>,
}
/// Untrusted deterministic simulation output. Cannot register a production identity.
pub struct Simulation {
    /// Simulated matching facts.
    pub matches: Vec<MatchFact>,
    /// Simulated fixture, only when requested.
    pub complete: Option<Private>,
}
impl std::fmt::Debug for VerifiedMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedMatch([redacted])")
    }
}
impl std::fmt::Debug for Projection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Projection([redacted])")
    }
}
impl std::fmt::Debug for Simulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Simulation([untrusted redacted])")
    }
}

/// Caller-selected identity transformation policy.
pub struct IdentityRequest {
    /// Comparison scope; unrelated scopes produce unlinkable outputs.
    pub scope: String,
    /// Captured face whose matching candidate is requested.
    pub face: Uuid,
    /// Caller-required confidence, from zero through one.
    pub minimum_confidence: f64,
}
