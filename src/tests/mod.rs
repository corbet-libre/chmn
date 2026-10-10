use crate::*;
use cfbk::cost::{
    Amount, CostOrder, DispatchPermit, Mode, Offer, Outcome, Plan, Price, Quote, Unit,
};
use cfbk::{Candidate, Config};
use futures::executor::block_on;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use uuid::Uuid;

fn face() -> Uuid {
    Uuid::from_u128(1)
}
fn person() -> Uuid {
    Uuid::from_u128(2)
}
fn capture() -> Capture {
    Capture {
        scope: "application".into(),
        subject: "private-member".into(),
        session: "capture-session".into(),
        operation: "operation".into(),
        faces: BTreeSet::from([face(), Uuid::from_u128(3)]),
        expires_at: 200,
    }
}
fn operation() -> Operation {
    Operation::VerifyPerson {
        face: face(),
        group: "group".into(),
        person: person(),
    }
}
fn adapter() -> AzureFace {
    AzureFace::new(
        "account".into(),
        "https://fixture.cognitiveservices.azure.com",
        "inert-test-subscription-key".into(),
        Mode::Production,
    )
    .unwrap()
}
fn amount(atoms: u64) -> Amount {
    Amount {
        unit: Unit {
            code: "USD".into(),
            scale: 6,
        },
        atoms,
    }
}
fn tariff() -> Tariff {
    Tariff {
        price: Price::AtMost(amount(2)),
        source: "fixture configured account tariff; not published price".into(),
        valid_until: 180,
    }
}
fn permit(quote: Quote, mode: Mode) -> DispatchPermit<String> {
    Plan::new(
        vec![Offer {
            candidate: Candidate::new("account".to_string(), [1]),
            quote,
        }],
        Config::new(["account".to_string()], [1]),
        CostOrder::Configured,
        amount(10),
        1,
        mode,
        100,
    )
    .unwrap()
    .start()
    .begin(100)
    .unwrap()
}
fn task() -> Task {
    Task {
        operation: operation(),
        capture: capture(),
        tariff: tariff(),
    }
}
const RESPONSE: &[u8] =
    br#"{"isIdentical":true,"confidence":0.92,"extension":{"private":"retained"}}"#;
#[derive(Default)]
struct Journal(Mutex<BTreeMap<String, String>>);
impl OperationStore for Journal {
    async fn reserve(
        &self,
        operation: &str,
        binding: &str,
        maximum: &Amount,
        valid_until: u64,
    ) -> Result<(), Error> {
        assert_eq!(maximum, &amount(2));
        if valid_until <= 100 {
            return Err(Error::Reservation);
        }
        let mut state = self.0.lock().unwrap();
        if state.contains_key(operation) {
            return Err(Error::Reservation);
        }
        state.insert(operation.into(), binding.into());
        Ok(())
    }
}
struct Dummy {
    response: Vec<u8>,
    status: u16,
    fail: bool,
    calls: Mutex<Vec<(String, serde_json::Value)>>,
}
impl Dummy {
    fn new(bytes: &[u8]) -> Self {
        Self {
            response: bytes.to_vec(),
            status: 200,
            fail: false,
            calls: Mutex::new(vec![]),
        }
    }
}
impl Transport for Dummy {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, Error> {
        assert_eq!(&*request.key, "inert-test-subscription-key");
        assert_eq!(format!("{request:?}"), "HttpRequest([redacted])");
        self.calls.lock().unwrap().push((
            request.url,
            serde_json::from_slice(request.body.expose()).unwrap(),
        ));
        if self.fail {
            return Err(Error::Uncertain);
        }
        Ok(HttpResponse {
            status: self.status,
            headers: Private::new(br#"[["x-provider",[112,114,105,118,97,116,101]]]"#.to_vec()),
            body: Private::new(self.response.clone()),
        })
    }
}
fn execute(dummy: &Dummy, operation: Operation) -> Result<VerifiedMatch, Error> {
    let a = adapter();
    let capture = capture();
    let tariff = tariff();
    let quote = a.preview(&operation, &capture, &tariff, 100)?;
    block_on(a.execute(
        dummy,
        &Journal::default(),
        Task {
            operation,
            capture,
            tariff,
        },
        permit(quote, Mode::Production),
        100,
    ))
}
fn identity_request() -> IdentityRequest {
    IdentityRequest {
        scope: "registrations:2026".into(),
        face: face(),
        minimum_confidence: 0.8,
    }
}

#[test]
fn real_request_mapping_and_private_rich_minimal_transformation() {
    let dummy = Dummy::new(RESPONSE);
    let result = execute(&dummy, operation()).unwrap();
    let calls = dummy.calls.lock().unwrap();
    assert_eq!(
        calls[0].0,
        "https://fixture.cognitiveservices.azure.com/face/v1.2/verify"
    );
    assert_eq!(
        calls[0].1,
        serde_json::json!({"faceId":face(),"personId":person(),"personGroupId":"group"})
    );
    let minimal = result
        .project(&Request::default(), &capture(), 100)
        .unwrap();
    assert!(minimal.complete.is_none());
    assert!(minimal.matches[0].confidence.is_none());
    assert!(minimal.matches[0].matched);
    let rich = result
        .project(
            &Request {
                complete: true,
                confidence: true,
                ..Request::default()
            },
            &capture(),
            100,
        )
        .unwrap();
    assert_eq!(rich.complete.as_ref().unwrap().expose(), RESPONSE);
    assert_eq!(rich.matches[0].confidence, Some(0.92));
    assert!(
        !format!("{result:?} {rich:?} {:?} {:?}", adapter(), capture()).contains("private-member")
    );
    assert!(!format!("{:?}", rich.complete.unwrap()).contains("retained"));
    let key = IdentityKey::new([7; 32]).unwrap();
    let identity = result
        .comparison(&key, &identity_request(), &capture(), 100)
        .unwrap();
    let mut request = identity_request();
    request.scope = "other".into();
    assert_ne!(
        identity.bytes(),
        result
            .comparison(&key, &request, &capture(), 100)
            .unwrap()
            .bytes()
    );
    assert!(!format!("{key:?} {identity:?}").contains("group"));
    let mut changed = capture();
    changed.subject = "different-member".into();
    assert!(result.project(&Request::default(), &changed, 100).is_err());
    assert!(result.comparison(&key, &request, &capture(), 200).is_err());
    request.minimum_confidence = 0.99;
    assert!(result.comparison(&key, &request, &capture(), 100).is_err());
}
#[test]
fn comparison_and_identification_preserve_supported_semantics() {
    let compare = Operation::Compare {
        first: face(),
        second: Uuid::from_u128(3),
    };
    let result = execute(&Dummy::new(RESPONSE), compare).unwrap();
    assert!(
        result
            .comparison(
                &IdentityKey::new([1; 32]).unwrap(),
                &identity_request(),
                &capture(),
                100
            )
            .is_err()
    );
    let op = Operation::Identify {
        faces: vec![face()],
        group: "group".into(),
        maximum_candidates: 2,
        threshold: 0.5,
    };
    let bytes=serde_json::to_vec(&serde_json::json!([{"faceId":face(),"candidates":[{"personId":person(),"confidence":0.9}],"extension":42}])).unwrap();
    let dummy = Dummy::new(&bytes);
    let result = execute(&dummy, op.clone()).unwrap();
    assert_eq!(
        dummy.calls.lock().unwrap()[0].0,
        "https://fixture.cognitiveservices.azure.com/face/v1.2/identify"
    );
    let key = IdentityKey::new([1; 32]).unwrap();
    assert!(
        result
            .comparison(&key, &identity_request(), &capture(), 100)
            .is_ok()
    );
    for candidates in [
        serde_json::json!([]),
        serde_json::json!([{"personId":person(),"confidence":0.9},{"personId":Uuid::from_u128(4),"confidence":0.85}]),
    ] {
        let bytes =
            serde_json::to_vec(&serde_json::json!([{"faceId":face(),"candidates":candidates}]))
                .unwrap();
        let result = execute(&Dummy::new(&bytes), op.clone()).unwrap();
        assert!(
            result
                .comparison(&key, &identity_request(), &capture(), 100)
                .is_err()
        );
    }
    let mismatch = execute(
        &Dummy::new(br#"{"isIdentical":false,"confidence":0.92}"#),
        operation(),
    )
    .unwrap();
    assert!(
        mismatch
            .comparison(&key, &identity_request(), &capture(), 100)
            .is_err()
    );
}
#[test]
fn malformed_or_unbound_provider_results_are_uncertain_after_dispatch() {
    for bytes in [
        b"invalid".as_slice(),
        br#"{}"#,
        br#"{"isIdentical":true,"confidence":2}"#,
        br#"{"isIdentical":true,"confidence":-1}"#,
    ] {
        assert_eq!(
            execute(&Dummy::new(bytes), operation()).unwrap_err(),
            Error::Uncertain
        );
    }
    let op = Operation::Identify {
        faces: vec![face()],
        group: "group".into(),
        maximum_candidates: 1,
        threshold: 0.5,
    };
    for value in [
        serde_json::json!([]),
        serde_json::json!([{"faceId":Uuid::from_u128(99),"candidates":[]}]),
        serde_json::json!([{"faceId":face(),"candidates":[{"personId":person(),"confidence":0.1}]}]),
        serde_json::json!([{"faceId":face(),"candidates":[{"personId":Uuid::nil(),"confidence":0.9}]}]),
        serde_json::json!([{"faceId":face(),"candidates":[{"personId":person(),"confidence":0.9},{"personId":person(),"confidence":0.9}]}]),
    ] {
        assert!(
            execute(
                &Dummy::new(&serde_json::to_vec(&value).unwrap()),
                op.clone()
            )
            .is_err()
        );
    }
    let mut dummy = Dummy::new(RESPONSE);
    dummy.status = 429;
    assert!(matches!(
        execute(&dummy, operation()).unwrap_err(),
        Error::ProviderResponse { status: 429, .. }
    ));
    dummy.status = 200;
    dummy.fail = true;
    assert_eq!(execute(&dummy, operation()).unwrap_err(), Error::Uncertain);
}
#[test]
fn authorization_rejects_changed_terms_expiry_mode_and_duplicate_dispatch() {
    let a = adapter();
    let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    for scenario in 0..5 {
        let mut task = task();
        let mut changed = q.clone();
        let mut mode = Mode::Production;
        let mut now = 100;
        match scenario {
            0 => task.capture.subject = "other".into(),
            1 => changed.price = Price::Exact(amount(0)),
            2 => mode = Mode::Sandbox,
            3 => now = 180,
            _ => task.capture.operation = "other".into(),
        }
        let dummy = Dummy::new(RESPONSE);
        assert!(
            block_on(a.execute(
                &dummy,
                &Journal::default(),
                task,
                permit(changed, mode),
                now
            ))
            .is_err()
        );
        assert!(dummy.calls.lock().unwrap().is_empty());
    }
    let journal = Journal::default();
    let dummy = Dummy::new(RESPONSE);
    assert!(
        block_on(a.execute(
            &dummy,
            &journal,
            task(),
            permit(q.clone(), Mode::Production),
            100
        ))
        .is_ok()
    );
    assert_eq!(
        block_on(a.execute(&dummy, &journal, task(), permit(q, Mode::Production), 100))
            .unwrap_err(),
        Error::Reservation
    );
    assert_eq!(dummy.calls.lock().unwrap().len(), 1);
    let mut unknown = tariff();
    unknown.price = Price::Unknown;
    let quote = a.preview(&operation(), &capture(), &unknown, 100).unwrap();
    assert!(
        Plan::new(
            vec![Offer {
                candidate: Candidate::new("account".to_string(), [1]),
                quote
            }],
            Config::new(["account".to_string()], [1]),
            CostOrder::Configured,
            amount(10),
            1,
            Mode::Production,
            100
        )
        .is_err()
    );
}
#[test]
fn configuration_capture_and_operation_validation() {
    for endpoint in [
        "invalid",
        "http://fixture.cognitiveservices.azure.com",
        "https://evil.example",
        "https://x.y.cognitiveservices.azure.com",
        "https://fixture.cognitiveservices.azure.com/path",
        "https://user@fixture.cognitiveservices.azure.com",
        "https://fixture.cognitiveservices.azure.com/?x=1",
        "https://fixture.cognitiveservices.azure.com/#x",
        "https://fixture.cognitiveservices.azure.com:8443",
    ] {
        assert!(
            AzureFace::new(
                "a".into(),
                endpoint,
                "inert-test-subscription-key".into(),
                Mode::Production
            )
            .is_err()
        );
    }
    assert!(
        AzureFace::new(
            "a".into(),
            "https://fixture.cognitiveservices.azure.com",
            "short".into(),
            Mode::Production
        )
        .is_err()
    );
    assert!(
        AzureFace::new(
            "a".into(),
            "https://fixture.cognitiveservices.azure.com",
            "inert-test-subscription-key".into(),
            Mode::Simulation
        )
        .is_err()
    );
    assert!(IdentityKey::new([0; 32]).is_err());
    for value in ["", "bad\nvalue"] {
        assert!(identifier(value).is_err());
    }
    assert!(identifier(&"x".repeat(1025)).is_err());
    let a = adapter();
    for op in [
        Operation::Compare {
            first: face(),
            second: face(),
        },
        Operation::VerifyPerson {
            face: Uuid::nil(),
            group: "group".into(),
            person: person(),
        },
        Operation::VerifyPerson {
            face: face(),
            group: "Upper".into(),
            person: person(),
        },
        Operation::VerifyPerson {
            face: face(),
            group: "group".into(),
            person: Uuid::nil(),
        },
        Operation::Identify {
            faces: vec![],
            group: "group".into(),
            maximum_candidates: 1,
            threshold: 0.5,
        },
        Operation::Identify {
            faces: vec![face(), face()],
            group: "group".into(),
            maximum_candidates: 1,
            threshold: 0.5,
        },
        Operation::Identify {
            faces: vec![face()],
            group: "group".into(),
            maximum_candidates: 0,
            threshold: 0.5,
        },
        Operation::Identify {
            faces: vec![face()],
            group: "group".into(),
            maximum_candidates: 1,
            threshold: f64::NAN,
        },
    ] {
        assert!(a.preview(&op, &capture(), &tariff(), 100).is_err());
    }
    let mut cap = capture();
    cap.faces.clear();
    assert!(a.preview(&operation(), &cap, &tariff(), 100).is_err());
    assert!(a.preview(&operation(), &capture(), &tariff(), 200).is_err());
    let mut t = tariff();
    t.valid_until = 100;
    assert!(a.preview(&operation(), &capture(), &t, 100).is_err());
    #[cfg(not(target_arch = "wasm32"))]
    assert!(ReqwestTransport::new().is_ok());
}
#[test]
fn local_simulation_is_a_distinct_untrusted_projection() {
    let result = simulate(
        operation(),
        capture(),
        RESPONSE.to_vec(),
        &Request {
            complete: true,
            confidence: true,
            ..Request::default()
        },
        100,
    )
    .unwrap();
    assert!(result.matches[0].matched);
    assert_eq!(result.complete.unwrap().expose(), RESPONSE);
    assert!(
        simulate(
            operation(),
            capture(),
            b"invalid".to_vec(),
            &Request::default(),
            100
        )
        .is_err()
    );
    // Shared cost orchestration handles failures/uncertainty; no transport is present.
    let quote = Quote {
        price: Price::Exact(amount(2)),
        source: "explicit dummy cost".into(),
        valid_until: 200,
        binding: "local-fixture".into(),
    };
    let mut cascade = Plan::new(
        vec![Offer {
            candidate: Candidate::new("account", [1]),
            quote,
        }],
        Config::new(["account"], [1]),
        CostOrder::Configured,
        amount(2),
        1,
        Mode::Simulation,
        100,
    )
    .unwrap()
    .start();
    cascade.begin(100).unwrap();
    cascade.complete(Outcome::Uncertain, None).unwrap();
    assert!(cascade.begin(100).is_err());
    assert_eq!(cascade.exposure(), amount(2));
    cascade.complete(Outcome::Success, Some(amount(2))).unwrap();
    assert!(cascade.begin(100).is_err());
}
type RegistrationState = BTreeMap<([u8; 32], String), (u64, BTreeSet<String>)>;
#[derive(Default)]
struct Registrations(Mutex<RegistrationState>);
impl RegistrationStore for Registrations {
    async fn reserve(
        &self,
        identity: &ScopedIdentity,
        scope: &str,
        maximum: u64,
        operation: &str,
    ) -> Result<(), Error> {
        let mut state = self.0.lock().unwrap();
        let entry = state
            .entry((*identity.bytes(), scope.into()))
            .or_insert_with(|| (maximum, BTreeSet::new()));
        if entry.0 != maximum {
            return Err(Error::Reservation);
        }
        if entry.1.contains(operation) {
            return Ok(());
        }
        if entry.1.len() as u64 >= maximum {
            return Err(Error::Reservation);
        }
        entry.1.insert(operation.into());
        Ok(())
    }
}
#[test]
fn concurrent_general_registration_limits_and_exact_retries() {
    let identity = execute(&Dummy::new(RESPONSE), operation())
        .unwrap()
        .comparison(
            &IdentityKey::new([1; 32]).unwrap(),
            &identity_request(),
            &capture(),
            100,
        )
        .unwrap();
    let store = Registrations::default();
    let admitted = std::thread::scope(|threads| {
        let jobs = (0..4)
            .map(|n| {
                let identity = &identity;
                let store = &store;
                threads.spawn(move || {
                    block_on(identity.register(
                        store,
                        &Registration {
                            policy_scope: "month".into(),
                            maximum: 2,
                            operation: n.to_string(),
                        },
                        100,
                    ))
                    .is_ok()
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|j| usize::from(j.join().unwrap()))
            .sum::<usize>()
    });
    assert_eq!(admitted, 2);
    let operation = store
        .0
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .1
        .first()
        .unwrap()
        .clone();
    assert!(
        block_on(identity.register(
            &store,
            &Registration {
                policy_scope: "month".into(),
                maximum: 2,
                operation: operation.clone()
            },
            100
        ))
        .is_ok()
    );
    assert!(
        block_on(identity.register(
            &store,
            &Registration {
                policy_scope: "month".into(),
                maximum: 3,
                operation
            },
            100
        ))
        .is_err()
    );
    assert!(
        block_on(identity.register(
            &store,
            &Registration {
                policy_scope: "next-month".into(),
                maximum: 2,
                operation: "new".into()
            },
            100
        ))
        .is_ok()
    );
    assert!(
        block_on(identity.register(
            &store,
            &Registration {
                policy_scope: "month".into(),
                maximum: 0,
                operation: "zero".into()
            },
            100
        ))
        .is_err()
    );
}

#[test]
fn account_boundary_and_extreme_operation_inputs_refuse_before_dispatch() {
    for endpoint in [
        "https://:password@fixture.cognitiveservices.azure.com",
        "https://.cognitiveservices.azure.com",
    ] {
        assert!(
            AzureFace::new(
                "account".into(),
                endpoint,
                "inert-test-subscription-key".into(),
                Mode::Production
            )
            .is_err()
        );
    }
    for key in ["x".repeat(4097), "inert-test-key\nprivate".into()] {
        assert!(
            AzureFace::new(
                "account".into(),
                "https://fixture.cognitiveservices.azure.com",
                key,
                Mode::Production
            )
            .is_err()
        );
    }
    let mut cap = capture();
    cap.faces = (1..=11).map(Uuid::from_u128).collect();
    assert!(cap.validate(100).is_err());
    let oversized = Operation::Identify {
        faces: (1..=11).map(Uuid::from_u128).collect(),
        group: "group".into(),
        maximum_candidates: 1,
        threshold: 0.5,
    };
    assert!(oversized.validate(&capture()).is_err());
    for group in ["".into(), "a".repeat(65)] {
        assert!(
            Operation::VerifyPerson {
                face: face(),
                person: person(),
                group
            }
            .validate(&capture())
            .is_err()
        );
    }
    assert!(
        Operation::VerifyPerson {
            face: face(),
            person: person(),
            group: "a1-_".into()
        }
        .validate(&capture())
        .is_ok()
    );
    let a = adapter();
    let quote = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    let other = Plan::new(
        vec![Offer {
            candidate: Candidate::new("different".to_string(), [1]),
            quote,
        }],
        Config::new(["different".to_string()], [1]),
        CostOrder::Configured,
        amount(10),
        1,
        Mode::Production,
        100,
    )
    .unwrap()
    .start()
    .begin(100)
    .unwrap();
    let dummy = Dummy::new(RESPONSE);
    assert_eq!(
        block_on(a.execute(&dummy, &Journal::default(), task(), other, 100)).unwrap_err(),
        Error::Refused
    );
    assert!(dummy.calls.lock().unwrap().is_empty());
    let sandbox = AzureFace::new(
        "account".into(),
        "https://fixture.cognitiveservices.azure.com",
        "inert-test-subscription-key".into(),
        Mode::Sandbox,
    )
    .unwrap();
    let quote = sandbox
        .preview(&operation(), &capture(), &tariff(), 100)
        .unwrap();
    let result = block_on(sandbox.execute(
        &dummy,
        &Journal::default(),
        task(),
        permit(quote, Mode::Sandbox),
        100,
    ))
    .unwrap();
    let projection = result
        .project(&Request::default(), &capture(), 100)
        .unwrap();
    assert_eq!(projection.mode, Mode::Sandbox);
    let key = IdentityKey::new([7; 32]).unwrap();
    assert_eq!(
        result
            .comparison(&key, &identity_request(), &capture(), 100)
            .unwrap_err(),
        Error::Refused
    );
}
#[test]
fn duplicate_provider_candidates_and_requested_projection_thresholds() {
    assert_eq!(
        execute(&Dummy::new(&vec![b'x'; 1_048_577]), operation()).unwrap_err(),
        Error::Uncertain
    );
    let op = Operation::Identify {
        faces: vec![face(), Uuid::from_u128(3)],
        group: "group".into(),
        maximum_candidates: 2,
        threshold: 0.5,
    };
    let duplicate_faces =
        serde_json::json!([{"faceId":face(),"candidates":[]},{"faceId":face(),"candidates":[]}]);
    assert!(
        execute(
            &Dummy::new(&serde_json::to_vec(&duplicate_faces).unwrap()),
            op
        )
        .is_err()
    );
    let op = Operation::Identify {
        faces: vec![face()],
        group: "group".into(),
        maximum_candidates: 2,
        threshold: 0.5,
    };
    let duplicate_people = serde_json::json!([{"faceId":face(),"candidates":[{"personId":person(),"confidence":0.9},{"personId":person(),"confidence":0.9}]}]);
    assert!(
        execute(
            &Dummy::new(&serde_json::to_vec(&duplicate_people).unwrap()),
            op
        )
        .is_err()
    );
    let result = execute(&Dummy::new(RESPONSE), operation()).unwrap();
    let p = result
        .project(
            &Request {
                references: true,
                minimum_confidence: 0.99,
                ..Request::default()
            },
            &capture(),
            100,
        )
        .unwrap();
    assert!(!p.matches[0].matched);
    assert_eq!(p.matches[0].face, Some(face()));
    let mismatch = execute(
        &Dummy::new(br#"{"isIdentical":false,"confidence":0.1}"#),
        operation(),
    )
    .unwrap();
    assert!(
        !mismatch
            .project(&Request::default(), &capture(), 100)
            .unwrap()
            .matches[0]
            .matched
    );
    let mut req = identity_request();
    req.face = Uuid::from_u128(100);
    assert!(
        result
            .comparison(&IdentityKey::new([1; 32]).unwrap(), &req, &capture(), 100)
            .is_err()
    );
    let simulated = simulate(
        operation(),
        capture(),
        RESPONSE.to_vec(),
        &Request::default(),
        100,
    )
    .unwrap();
    assert_eq!(format!("{simulated:?}"), "Simulation([untrusted redacted])");
}
#[test]
fn uncertain_operations_and_concurrent_attempts_keep_one_host_reservation() {
    let a = adapter();
    let journal = Journal::default();
    let mut dummy = Dummy::new(RESPONSE);
    dummy.fail = true;
    let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    assert_eq!(
        block_on(a.execute(
            &dummy,
            &journal,
            task(),
            permit(q.clone(), Mode::Production),
            100
        ))
        .unwrap_err(),
        Error::Uncertain
    );
    assert_eq!(
        block_on(a.execute(&dummy, &journal, task(), permit(q, Mode::Production), 100))
            .unwrap_err(),
        Error::Reservation
    );
    assert_eq!(dummy.calls.lock().unwrap().len(), 1);
    let dummy = Dummy::new(RESPONSE);
    let journal = Journal::default();
    let accepted = std::thread::scope(|s| {
        let jobs = (0..4)
            .map(|_| {
                let a = &a;
                let dummy = &dummy;
                let journal = &journal;
                s.spawn(move || {
                    let q = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
                    block_on(a.execute(dummy, journal, task(), permit(q, Mode::Production), 100))
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|j| usize::from(j.join().unwrap()))
            .sum::<usize>()
    });
    assert_eq!(accepted, 1);
    assert_eq!(dummy.calls.lock().unwrap().len(), 1);
}

#[test]
fn retained_comparison_handles_do_not_register_after_capture_expiry() {
    let id = execute(&Dummy::new(RESPONSE), operation())
        .unwrap()
        .comparison(
            &IdentityKey::new([1; 32]).unwrap(),
            &identity_request(),
            &capture(),
            100,
        )
        .unwrap();
    let request = Registration {
        policy_scope: "window".into(),
        maximum: 1,
        operation: "op".into(),
    };
    assert_eq!(
        block_on(id.register(&Registrations::default(), &request, 200)),
        Err(Error::Rejected)
    );
}

#[test]
fn upstream_failures_remain_private_until_explicitly_requested() {
    let mut dummy =
        Dummy::new(br#"{"error":{"message":"private-provider-detail"},"extension":42}"#);
    dummy.status = 403;
    let error = execute(&dummy, operation()).unwrap_err();
    assert_eq!(error.upstream().unwrap().expose(), dummy.response);
    assert!(!format!("{error:?} {error}").contains("private-provider-detail"));
    assert!(Error::Uncertain.upstream().is_none());
}

#[test]
fn reservation_uses_a_fresh_host_clock_and_refuses_expired_dispatch() {
    struct Expired;
    impl OperationStore for Expired {
        async fn reserve(&self, _: &str, _: &str, _: &Amount, deadline: u64) -> Result<(), Error> {
            assert_eq!(deadline, 180);
            Err(Error::Reservation)
        }
    }
    let a = adapter();
    let quote = a.preview(&operation(), &capture(), &tariff(), 100).unwrap();
    let dummy = Dummy::new(RESPONSE);
    assert_eq!(
        block_on(a.execute(
            &dummy,
            &Expired,
            task(),
            permit(quote, Mode::Production),
            100
        ))
        .unwrap_err(),
        Error::Reservation
    );
    assert!(dummy.calls.lock().unwrap().is_empty());
}

#[test]
fn explicit_report_retains_bounded_replies_without_authenticating_invalid_data() {
    for (status, body, accepted) in [
        (200, RESPONSE.to_vec(), true),
        (200, b"private-malformed-response".to_vec(), false),
        (
            200,
            br#"{"isIdentical":true,"confidence":4.2,"extension":"private"}"#.to_vec(),
            false,
        ),
        (403, b"private-provider-refusal".to_vec(), false),
    ] {
        let adapter = adapter();
        let quote = adapter
            .preview(&operation(), &capture(), &tariff(), 100)
            .unwrap();
        let mut wire = Dummy::new(&body);
        wire.status = status;
        let report = block_on(adapter.execute_report(
            &wire,
            &Journal::default(),
            task(),
            permit(quote, Mode::Production),
            100,
        ));
        assert_eq!(report.result.is_ok(), accepted);
        if let Ok(verified) = report.result {
            assert_eq!(verified.upstream().expose(), body);
            let invalid = Request {
                minimum_confidence: 2.0,
                ..Request::default()
            };
            assert!(verified.project(&invalid, &capture(), 100).is_err());
            assert_eq!(verified.upstream().expose(), body);
        }
        let raw = report.response.unwrap();
        assert_eq!(raw.status, status);
        let headers: Vec<(String, Vec<u8>)> = serde_json::from_slice(raw.headers.expose()).unwrap();
        assert_eq!(headers, vec![("x-provider".into(), b"private".to_vec())]);
        assert_eq!(raw.body.expose(), body);
        assert_eq!(wire.calls.lock().unwrap().len(), 1);
    }
    for fail in [true, false] {
        let adapter = adapter();
        let quote = adapter
            .preview(&operation(), &capture(), &tariff(), 100)
            .unwrap();
        let mut wire = Dummy::new(&vec![0; 1_048_577]);
        wire.fail = fail;
        let report = block_on(adapter.execute_report(
            &wire,
            &Journal::default(),
            task(),
            permit(quote, Mode::Production),
            100,
        ));
        assert!(report.result.is_err());
        assert!(report.response.is_none());
    }
}
