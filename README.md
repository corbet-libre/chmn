# chmn

**Biometric admission gate for cvld.**

`chmn` will prove that a member is a living human through an established biometric verification provider. The gate itself never sees raw biometric data; `cvld` receives only an opaque proof and its trust anchor.

Status: name reserved, no implementation yet.

## Boundaries

What it does:

- Checks a provider's liveness/uniqueness result and returns an opaque gate proof with its trust anchor.

What it never does:

- Never receives, stores or forwards raw biometric data (images, templates, video).
- Raw factor data never reaches `cvld`: it receives only an opaque proof with its trust anchor. No raw data in output, logs, errors or fixtures.
- No own security primitives: a thin wrapper around the established best-practice library for the factor.

## License

Copyright 2026 Julian Y. Richard Corbet. Licensed under
[LGPL-3.0-only](LICENSES/LGPL-3.0-only.txt)
[WITH LGPL-3.0-linking-exception](LICENSES/LGPL-3.0-linking-exception.txt).
See [LICENSE.md](LICENSE.md).
