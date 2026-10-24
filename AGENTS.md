# Agent instructions

Write code and documentation in English. Follow the reusable domain-library
charter in the shared brain; this supersedes the original cvld-only liveness
placeholder scope. Own only safe Rust. Integrate maintained implementations.

- Biometrics and provider-supported matching are the domain, not a claim of
  universal personhood. Preserve actual provider/account/group guarantees.
- Caller-requested full results are allowed through the private RAM API. cvld
  requests only its policy facts. No direct DB, persistence or raw diagnostics.
- Credentials and stable comparison keys are injected explicitly and separately.
- Potentially billable calls require a bounded cost permit and atomic caller
  operation/budget reservation. No automatic retry or provider activation.
- Never spend, sign up or run live provider calls for development or tests.
- Keep LGPL-3.0-only WITH LGPL-3.0-linking-exception compatibility.
- Local checkouts edit/format; Crow runs fmt, clippy, tests, WASM and the existing
  complete source/branch coverage gate. Do not weaken checks to claim completion.
- Committed main candidates enable remote testing. No deployment or publication.
