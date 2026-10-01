# Human gate boundary

This reserved gate checks one established provider's liveness/uniqueness result
for an opaque member. The configured provider authenticates that result; this
library must not create a confidence ranking or its own biometric/cryptographic
primitive. `cfbk` owns ordering and fallback under `crbk` requirements.

Raw images, templates and video never enter this library, cvld, attestation
output, logs, errors or fixtures. No provider adapter is implemented or
qualified. Tests are fixture-only and do not authorize live provider calls,
registration, spending or outreach. Empty production coverage must fail until
a real adapter and its adversarial tests exist.
