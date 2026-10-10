# Implementation acceptance contract

`chmn` verifies evidence about a human. Product rules choose whether that evidence
is sufficient for an action. Implement this as an adapter boundary, not a fixed
provider list or a policy engine. A provider being implemented does not enable it.

## Required deliverables

| Deliverable | Acceptance requirement |
|---|---|
| Provider contract | Advertise implemented proof capabilities and their scope. Select one explicitly configured provider per invocation. Keep provider selection, ordering and fallback in their existing owners. |
| Authenticated input | Accept a provider proof, token or correlated verification-session reference. Authenticate it through a maintained verifier or the provider's authenticated API. A member-supplied boolean or unsigned JSON report is not evidence. |
| Context binding | Bind the verified result to the authenticated subject, service/application, requested action and fresh challenge/check. Verify issuer, audience, nonce, expiry and signature/protocol constraints where the protocol provides them. Otherwise use a server-created, durably correlated provider session; never accept an arbitrary member-selected session ID. |
| Explicit facts | Distinguish liveness, human verification and uniqueness. A live face does not prove one account per person. A successful session does not imply every capability the provider offers was checked. |
| Minimal output | Produce an opaque in-process verified value with provider, context, fact and bounded validity. Client deserialization must not manufacture this verified value. Keep provider responses and session identifiers inside the leaf. |
| Uniqueness | Produce a stable canonical identity only when authenticated evidence establishes the required uniqueness scope. Declare whether it is provider-, application- or action-scoped. Different sessions, encodings or unsigned metadata must not create new identities for the same claim. Never claim equivalence across unrelated providers without a verified common identity scheme. |
| Lifecycle | Model unfinished, refused, expired and uncertain verification separately. If a provider call can create a billable session, journal before dispatch and reuse its stable idempotency key; reconcile read-only after uncertainty. A retry must not silently create another billable session. |
| Provider implementation | Include at least one real adapter with maintained protocol/cryptographic dependencies and documented configuration. A generic trait plus a fixture adapter does not complete the library. Additional providers implement the same boundary. |
| Gate integration | Demonstrate the real `cgts::Gate` and `cgbl::GlobalGate` paths with fixture-backed verified evidence, including rejection cases. Do not move provider calls or raw responses into either facade. |
| Documentation | Document capabilities, exact proof scope, required credentials, limits, retained fields, expiry, replay behavior, erasure and provider-specific restrictions. |

## Information boundary

Raw biometric images, videos, templates and embeddings never enter `chmn`.
Capture and biometric processing stay with the selected device/provider flow.
This is stricter than the payment leaf's permission to inspect transient billing
data. An API requiring a selfie upload through this library is not compatible.

Provider tokens and opaque verification references may be needed transiently.
Treat them as private: redacted diagnostics, uniform errors, no request logging,
no serialization into application results, and zeroization of owned secret
buffers where supported. SDK/parser copies must be described honestly.
Persist only necessary correlation/replay/uniqueness data with explicit lifetimes;
erasure must not re-enable replay. No raw biometric fixtures.

## Existing gate seams

`cgts` has `Gate::verify(Context, Input) -> Proof`. The context includes the
authenticated community, subject, action and time. Its `Claim` is atomically
spent with the result. Use a stable proof/nullifier identity for one-use evidence;
a newly generated session ID is not automatically a canonical uniqueness claim.

`cgbl` has `GlobalGate::verify(CheckId, Subject, input, now) -> GateEvidence`.
Its evidence contains validity and an optional transient canonical uniqueness
input, which the facade fingerprints and wipes. Return `None` when the verified
capability does not establish the configured uniqueness requirement. The global
facade's existing policy decides whether that is sufficient. Canonicalize within
the actual verified identity domain; naming unrelated provider IDs alike cannot
make them the same person.

Respect each facade's time convention: `cgbl` currently uses inclusive expiry
and rounds downward to UTC days. Convert an exclusive provider expiry without
extending it. Provider expiry and the caller's requested lifetime both cap the
result. Liveness is an observation at a time, not an unlimited future guarantee.

## Credentials

The service composition root obtains credentials and constructs the adapter.
`chmn` must not discover environment variables, read a secrets store, choose an
account, register with a provider or request credentials from the member.
Support explicit SDK/client or credential-provider injection when an upstream
SDK needs refreshing tokens or signed requests.

Keep public application IDs, expected issuer/audience, endpoints and verification
keys separate from private API keys, OAuth client secrets and signing credentials.
Only provider-documented public IDs and short-lived, scoped ceremony tokens may
reach the member's device. Never forward an operator API secret. Production and
sandbox configurations are distinct and selected explicitly.

The host owns credential refresh and replacement. Updating a provider credential
must preserve pending-session correlation and replay identity. Persistent HMAC
keys are separate from replaceable provider credentials; changing them requires
an explicit migration, not an automatic fallback. Document every adapter's
credential type and required permissions. Tests inject inert fixtures only.

## Required refusal and lifecycle tests

- Forged/unsigned evidence, wrong issuer/audience/provider and modified payload.
- Another subject, service, action or challenge; replay under another gate alias.
- Expired/not-yet-valid proofs, boundary timestamps and caller/provider lifetime caps.
- Liveness-only evidence presented as uniqueness; action-scoped uniqueness
  presented as application/global uniqueness.
- Duplicate person/nullifier through changed sessions or unsigned metadata.
- Timeout, cancellation, malformed/oversized response, storage failure and restart.
- Concurrent retry and provider idempotency; read-only reconciliation cannot
  create a second session or billable action.
- Raw provider/KYC fields absent from outputs, retained state, Debug, errors and logs.
- Missing credentials, credential replacement, rejected redirects and explicit endpoints.
- Real community/global gate recording, claim consumption and uniqueness refusal.

Use current compatible dependency ranges and first-party `main`, with a committed
CI-tested lockfile. Fixture tests must exercise the actual provider SDK or wire
protocol. Crow runs native checks, applicable Wasm checks and the existing full
source-coverage gate. No live ceremonies, account registrations or paid calls are
authorized by this implementation contract. Publish implementation status and
limitations accurately; a placeholder cannot be labelled complete.
