# Provider and protocol catalogue

## Provider and protocol arsenal

Primary documentation reviewed 2026-10-10. **Implemented** means the local
fixture-tested path described here, not live-provider qualification.
**Extension-compatible** means the existing trusted interface can carry the
integration; provider-specific authentication/correlation still needs work.
**Candidate** means researched only. **Unsupported** means the stated guarantee
must not be inferred. These are integration options, not activated services.

Every future network route needs an explicit injected credential, account/mode
binding, bounded cost quote with provenance and expiry, atomic host reservations,
fresh dispatch deadlines, rich private reports and negative fixtures. Unknown
prices block dispatch. Cost bases below identify what a host must obtain; they
are not current numeric quotations or permission to contact the provider.

| Ecosystem and source | Status / reuse path | Credentials and cost basis | Semantic limit |
|---|---|---|---|
| [Azure Face](https://learn.microsoft.com/en-us/azure/ai-services/face/overview-identity) | Implemented Face v1.2 compare, person verification and group identification using maintained reqwest. | Inject resource endpoint/subscription key and account tariff covering the exact operation; access eligibility is provider-controlled. | Existing capture and enrolled groups are caller/provider responsibilities. Matching is probabilistic and scoped to account/group; liveness is separate. |
| [Amazon Rekognition face search](https://docs.aws.amazon.com/rekognition/latest/APIReference/API_SearchFacesByImage.html) | Candidate: prefer official `aws-sdk-rekognition`; compare/search plus separately selected [liveness](https://docs.aws.amazon.com/rekognition/latest/dg/face-liveness.html). | Inject scoped IAM credentials/session tokens and region; quote API, optional liveness and collection/storage costs independently. | Collection identity/model versions matter. A matched face or passed liveness check does not establish universal person uniqueness. |
| [iProov enrolled-user verification](https://docs.iproov.com/implementation/api/flow/verifier) | Candidate: official client SDK and server token/validate flow. | [Service API key/secret and configured region](https://docs.iproov.com/implementation/api/prepare); contracted transaction quote. | Match refers to the enrolled user. Validate is consumptive; preserve necessary host references atomically and never repeat uncertain validation. Raw provider data still stays out of this library's storage. |
| [FaceTec](https://dev.facetec.com/configuration-options) | Candidate: licensed device/server SDK integration via an authenticated host service; deployment needs qualification. | Customer SDK/server license and approved service authentication; obtain an explicit deployment/transaction quote before activation. | [Default integrations store FaceMaps](https://dev.facetec.com/3d-liveness). That persistence cannot run inside this RAM-only library; host/provider retention must be explicit. Liveness-only mode provides no face match. |

No capture UI, enrollment algorithm, biometric template database or universal
cross-provider matching is implemented here. Proprietary SDK candidates need
licence, authentication, privacy, persistence and safe-Rust-boundary evaluation
before integration. Keep any native implementation behind a reviewed external
service where required; never add owned unsafe code or hidden SDK storage.
