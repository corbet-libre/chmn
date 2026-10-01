# chmn

**Biometric admission gate for cvld.**

`chmn` will prove that a member is a living human through an established biometric verification provider. The gate itself never sees raw biometric data; `cvld` receives only an opaque proof bound to its configured provider.

Status: name reserved, no implementation or qualified provider adapter yet.
One invocation checks one provider result; `cfbk` owns ordering/fallback and
`crbk` owns requirements. The empty production coverage inventory must fail.
See [the current contract](docs/CONTRACT.md).

## Boundaries

What it does:

- Checks a provider's liveness/uniqueness result and returns an opaque gate proof bound to its configured provider.

What it never does:

- Never receives, stores or forwards raw biometric data (images, templates, video).
- Raw factor data never reaches `cvld`: it receives only an opaque proof bound to its configured provider. No raw data in output, logs, errors or fixtures.
- No own security primitives: a thin wrapper around the established best-practice library for the factor.

## License

Copyright 2026 Julian Y. Richard Corbet. Licensed under
[LGPL-3.0-only](LICENSES/LGPL-3.0-only.txt)
[WITH LGPL-3.0-linking-exception](LICENSES/LGPL-3.0-linking-exception.txt).
See [LICENSE.md](LICENSE.md).

## Continuous verification

Dependency updates follow main and are tested against one CI-resolved lockfile.
Line and branch coverage target 100%; failures remain blocking. See
[the coverage contract](docs/COVERAGE.md) for measurement and exclusions.
