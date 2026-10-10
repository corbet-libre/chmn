# chmn

Reusable, safe-Rust biometric provider integration. Azure Face v1.2 currently
supports face-to-face comparison, verification against an enrolled person, and
identification within an account's person group. Maintained `reqwest` handles
HTTPS, `uuid` parses provider IDs, and RustCrypto supplies keyed scoped identity
transformation. This crate implements neither biometric recognition algorithms
nor capture UI, enrollment, liveness or universal human uniqueness.

The caller supplies an authenticated capture binding, task, permitted account,
explicit credentials, verified tariff, total budget and atomic operation journal.
`AzureFace::preview` is inert; `cfbk::cost::Plan` selects and bounds a cascade;
`AzureFace::execute` checks its move-only permit again immediately before reserve
and dispatch. Unknown prices cannot dispatch. No automatic retries occur.

Results remain private in RAM. `VerifiedMatch::project` returns requested match
facts with optional caller confidence threshold; provider face references and
confidence are opt-in. Complete upstream JSON including unknown extensions is
explicitly available. The normal cvld path requests match predicates only.
`comparison` derives an opaque scoped identity only for one qualifying enrolled
person candidate, with account, endpoint, mode and group separation. The host
provides atomic X-registration limits and decides all windows/release policy.

See [the contract](docs/CONTRACT.md) for credentials, costs, modes, privacy and
qualification boundaries. No live provider call or account signup is required
by the fixture suite. Live provider compatibility is unqualified until an
explicitly authorized host runs its contract probes with its own account.

See the [provider and protocol catalogue](docs/PROVIDERS.md) for implemented paths,
research candidates, credentials, cost bases and unsupported guarantees.
