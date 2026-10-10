# Domain contract

## Working integration and candidates

The Azure Face v1.2 integration constructs the documented verify and identify
requests, parses provider match/candidate results, preserves extensions, and
refuses response face IDs not present in the authenticated request. The native
transport uses maintained reqwest over HTTPS to explicit public Azure resource
hosts; redirects, retries and automatic proxy discovery are disabled. WASM
consumers inject a trusted transport. There is no direct database client.

Implemented protocols:
- [Face-to-face verification](https://learn.microsoft.com/en-us/rest/api/face/face-recognition-operations/verify-face-to-face?view=rest-face-v1.2)
- [Person-group verification](https://learn.microsoft.com/en-us/rest/api/face/face-recognition-operations/verify-from-person-group?view=rest-face-v1.2)
- [Person-group identification](https://learn.microsoft.com/en-us/rest/api/face/face-recognition-operations/identify-from-person-group?view=rest-face-v1.2)

AWS Rekognition face collections, provider liveness and other enrollment/search
systems are researched candidates, not working adapters. The Azure crate name
alone did not supply a mature Face client; explicit REST over maintained reqwest
avoids claiming that an empty SDK package is an integration.

## Meaning and identity

Matching is a provider assessment, with provider confidence returned only on
request. Caller thresholds remain caller policy. Matching does not prove
liveness, legal identity, or that one natural person has only one enrollment.
Person-group identities are provider/account/endpoint/mode/group/person-scoped.
The library derives keyed, length-framed opaque comparison values; it never
merges unrelated accounts or guesses person identity from face IDs. An ambiguous
candidate set refuses comparison instead of selecting the highest score silently.

`Capture` is constructed only by a trusted host after it binds the face IDs to
its authenticated capture/session and subject. Accepting member-supplied face
IDs or member-built Capture values would bypass that host boundary. The capture
expiry is caller-supplied and must also respect provider face-ID validity.
`RegistrationStore` is an atomic host port for up to X registrations. Exact
retries are idempotent; changed terms or exhausted counts refuse. Windows,
releases and resets belong to caller policy, not a volatile in-library counter.

## Credentials, costs and execution

The host retrieves a subscription key from its own secret manager and injects it
into `AzureFace::new`; no environment scan or SDK credential discovery occurs.
Comparison `IdentityKey` is separately injected, stable key material. Rotating a
provider credential must not rotate identity comparisons. Never give either key
to members or publish it in diagnostic output.

The host supplies `Tariff` for the exact task, account, region and tier. A source
must identify its verified quote/configuration revision, price unit and validity.
No current Azure price is hardcoded. Exact or defensible maximum fees must cover
failed-attempt billing. `Price::Unknown` cannot authorize dispatch. Sandbox
charges remain bounded explicitly; an endpoint named sandbox is not assumed free.
Local compute and host costs are separate from provider API charges.

`preview` hashes provider/account/endpoint/mode/operation, every task parameter,
capture subject/session, validity and operation reference. Dispatch compares the
complete current quote with the permit. The trusted `OperationStore` atomically
reserves the operation identity and maximum shared budget before transport. It
must reject any previously dispatched operation, including exact retries, until
the host reconciles it. A process restart must not reset an uncertain operation.
Only opaque operation/binding/cost references go through the port, not raw data.

Transport errors and malformed success payloads leave the result uncertain.
A definitive HTTP rejection can still be billed. The caller reports that fact
to cfbk and retains maximum exposure unless authenticated billing evidence or its
verified tariff establishes actual cost. No automatic retry or fallback occurs
on uncertainty. Provider mode is carried in projections and identity domains.

`simulate` is credential-free and has no transport. It runs the same request and
result transformations against explicit untrusted fixtures and returns a distinct
`Simulation`, which cannot supply a production identity. The cfbk planner can be
run with dummy bounded tariffs/outcomes for budget and fallback simulation.

## RAM and verification boundary

Keys and original byte buffers zeroize on drop. serde, HTTP and crypto upstreams
can make temporary copies; host swap/core dumps remain host controlled. This is
an application-level no-persistence boundary, not guaranteed total memory erasure.
Private results and transport diagnostics redact Debug and errors. Caller code
owns copies requested through `expose` and chooses any persistence outside chmn.

Tests use exact documented JSON schemas through a dummy transport and the real
request construction, cost checking, transformations and RustCrypto identity
code. Concurrent registration and operation rejection tests exercise host-port
contracts. Live cloud calls are not run; provider access, enrollment quality,
biometric accuracy, liveness, retention and service availability are not certified
by the fixture suite. Production qualification needs explicit operator permission,
a current tariff, an approved account and provider-specific contract probes.

Sandbox matching results can be projected privately but cannot derive a
production registration identity. Non-success HTTP responses retain their exact
private body in `ProviderResponse`; only explicit `Error::upstream` access
exposes it. Default diagnostics redact all provider error bytes.

`OperationStore` receives the effective exclusive deadline and checks its fresh
host clock before authorizing dispatch; the planning timestamp cannot extend
an expired quote while a reservation is waiting.

`execute_report` explicitly preserves every bounded available reply, including
malformed and predicate-refused success bodies, without changing the minimal
`execute` result or authenticating arbitrary data. `VerifiedMatch::upstream`
allows private inspection after a projection/identity request refuses. Reports
have no revealing diagnostics; network failures and over-limit bodies have no
retained reply. No extra provider call is made.
