# Agent instructions

Write all code comments and documentation in English.

## Product boundary

- Biometric admission gate for cvld: proof of a living human without ever seeing raw biometric data.
- Checks a provider's liveness/uniqueness result and returns an opaque gate proof bound to its configured provider.
- Never receives, stores or forwards raw biometric data (images, templates, video).
- Raw factor data never reaches `cvld`: it receives only an opaque proof bound to its configured provider. No raw data in output, logs, errors or fixtures.
- No own security primitives: a thin wrapper around the established best-practice library for the factor.
- This crate is LGPL-3.0-only WITH LGPL-3.0-linking-exception. Do not add implementation code available only under the full GPL or AGPL.
- Tests use fixtures only; no live calls to third-party providers.

## Quality boundary

- Follow `docs/IMPLEMENTATION.md` for deliverables and credential ownership.
- `cargo fmt --check`, `cargo clippy --all-targets` (no warnings), `cargo test`
  and the existing coverage gate must pass before completion. Committed
  candidates enable remote validation. Workstations edit and format; Crow
  runs builds and tests. Forgejo is primary; no registry publication.
